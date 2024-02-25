use std::ffi::c_uint;
use std::fs::{self};
use std::io::Write;
use std::path::PathBuf;

use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::{io, thread, vec};

use anyhow::{anyhow, Context, Result};

use clap::Parser;

use gilrs::{Button, Gilrs};

use libretro_sys::{PixelFormat, DEVICE_JOYPAD};
use rodio::Source;

use crate::audio::RetroAudio;
use crate::core::{Callbacks, Core};
use crate::video::Frame;

mod audio;
pub mod core;
mod environment;
mod gui;
mod hook;
mod remote;
mod video;

#[derive(clap::Parser)]
struct Cli {
    #[clap(long, env = "APE_CORE")]
    core: PathBuf,
    #[clap(long, env = "APE_ROM")]
    rom: PathBuf,
}

fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let cli = Cli::parse();

    gui::run(cli).context("failed to run gui")?;

    Ok(())
}

fn run(
    core: impl Into<PathBuf>,
    rom: impl Into<PathBuf>,
    egui_ctx: egui::Context,
) -> Result<(Receiver<Option<Frame>>, hook::Handle)> {
    let core = core.into();
    let rom = rom.into();

    let (frame_tx, frame_rx) = sync_channel(1);
    let (audio_tx, audio_rx) = sync_channel(1);
    let (command_tx, command_rx) = sync_channel(32);

    let hook_host = hook::Host::new();
    let core_handle = hook_host.handle();

    thread::spawn(move || {
        let (_stream, stream_handle) = rodio::OutputStream::try_default()?;

        let gilrs = Gilrs::new()
            .map_err(|err| anyhow!("{err}"))
            .context("failed to initialize gilrs")?;

        let sram_path = rom.with_extension("sram");

        let callbacks = ApeCallbacks {
            frame_tx,
            audio_tx,
            gilrs,
            input_state: 0,
            command_tx,
            egui_ctx,
        };

        let core_config = core::Config {
            core,
            rom,
            callbacks: callbacks.boxed(),
        };

        let mut last_sram_save = Instant::now();
        let mut speed_factor = 1;

        Core::load(core_config, |core| -> Result<()> {
            match fs::read(&sram_path) {
                Ok(sram) => {
                    eprintln!("Restoring SRAM from {sram_path:?}");
                    core.restore_save_ram(&sram);
                }
                Err(err) => {
                    if err.kind() == io::ErrorKind::NotFound {
                        eprintln!("No SRAM file found at {sram_path:?}");
                    } else {
                        eprintln!("Failed to read SRAM from {sram_path:?}");
                    }
                }
            }

            remote::start(hook_host.handle());

            let system_av_info = core.get_system_av_info();

            println!("{:#?}", system_av_info);
            // panic!("sample rate: {}", system_av_info.timing.sample_rate);

            let sample_rate = Arc::new(RwLock::new(
                system_av_info.timing.sample_rate as u32 * speed_factor,
            ));

            let retro_audio = RetroAudio {
                rx: audio_rx,
                current_frame: Vec::new().into_iter(),
                sample_rate: sample_rate.clone(),
            };

            thread::spawn(move || {
                let res = stream_handle
                    .play_raw(retro_audio.convert_samples())
                    .context("failed to play stream");

                if let Err(err) = res {
                    eprintln!("Error while playing audio: {err}");
                }
            });

            let mut saved_state = None;

            loop {
                hook_host.run(core);

                if last_sram_save.elapsed() >= Duration::from_secs(5) {
                    if let Err(err) = core.save_sram_to(&sram_path) {
                        eprintln!("Failed to save SRAM: {err:?}");
                    }

                    last_sram_save = Instant::now();
                }

                if let Ok(command) = command_rx.try_recv() {
                    match command {
                        Command::SaveState => match core.state() {
                            Ok(state) => saved_state = Some(state),
                            Err(err) => eprintln!("{err:?}"),
                        },
                        Command::LoadState => {
                            if let Some(state) = &saved_state {
                                if let Err(err) = core.restore_state(state) {
                                    eprintln!("{err:?}")
                                }
                            }
                        }
                        Command::ToggleTurbo => {
                            speed_factor = match speed_factor {
                                1 => 2,
                                _ => 1,
                            };

                            *sample_rate.write().unwrap() =
                                system_av_info.timing.sample_rate as u32 * speed_factor;
                        }
                    }
                }
            }

            if let Err(err) = core.save_sram_to(&sram_path) {
                eprintln!("Failed to save SRAM: {err:?}");
            }

            Ok(())
        })
        .context("failed to load core")?
        .context("runtime error")?;

        println!("Exiting normally");

        anyhow::Ok(())
    });

    Ok((frame_rx, core_handle))
}

struct ApeCallbacks {
    frame_tx: SyncSender<Option<Frame>>,
    audio_tx: SyncSender<Vec<i16>>,
    gilrs: Gilrs,
    input_state: i16,
    command_tx: SyncSender<Command>,
    egui_ctx: egui::Context,
}

impl Callbacks for ApeCallbacks {
    fn video_refresh(&mut self, frame: Option<Frame>) {
        if self.frame_tx.try_send(frame).is_err() {
            eprintln!("Dropping frame, failed to send");
        }

        self.egui_ctx.request_repaint();
    }

    fn supports_pixel_format(&mut self, pixel_format: PixelFormat) -> bool {
        match pixel_format {
            PixelFormat::ARGB8888 => true,
            PixelFormat::RGB565 => true,
            PixelFormat::ARGB1555 => false,
        }
    }

    fn audio_sample(&mut self, left: i16, right: i16) {
        // TODO: avoid vec, probably use enum
        self.audio_tx.send(vec![left, right]).ok();
    }

    fn audio_samples(&mut self, samples: &[i16]) {
        self.audio_tx.send(samples.to_vec()).ok();
    }

    fn input_poll(&mut self) {
        while let Some(event) = self.gilrs.next_event() {
            let mut release = false;
            let button = match event.event {
                gilrs::EventType::ButtonPressed(button, _) => button,
                gilrs::EventType::ButtonReleased(button, _) => {
                    release = true;
                    button
                }
                _ => continue,
            };

            // if release {
            //     eprintln!("Released button {button:?}");
            // } else {
            //     eprintln!("Pressed button {button:?}");
            // }

            let button = match button {
                Button::South => libretro_sys::DEVICE_ID_JOYPAD_A, // libretro_sys::DEVICE_ID_JOYPAD_B,
                Button::West => libretro_sys::DEVICE_ID_JOYPAD_B, // libretro_sys::DEVICE_ID_JOYPAD_Y,
                Button::East => continue, // libretro_sys::DEVICE_ID_JOYPAD_A,
                Button::North => continue, // libretro_sys::DEVICE_ID_JOYPAD_X,
                Button::C => continue,
                Button::Z => continue,
                Button::LeftTrigger => {
                    // libretro_sys::DEVICE_ID_JOYPAD_L
                    libretro_sys::DEVICE_ID_JOYPAD_X
                    // self.command_tx.try_send(Command::LoadState).ok();
                    // continue;
                }
                Button::LeftTrigger2 => libretro_sys::DEVICE_ID_JOYPAD_L2,
                Button::RightTrigger => {
                    // libretro_sys::DEVICE_ID_JOYPAD_R
                    self.command_tx.try_send(Command::ToggleTurbo).ok();
                    continue;
                }
                Button::RightTrigger2 => libretro_sys::DEVICE_ID_JOYPAD_R2,
                Button::Select => libretro_sys::DEVICE_ID_JOYPAD_SELECT,
                Button::Start => libretro_sys::DEVICE_ID_JOYPAD_START,
                Button::Mode => continue,
                Button::LeftThumb => libretro_sys::DEVICE_ID_JOYPAD_L3,
                Button::RightThumb => libretro_sys::DEVICE_ID_JOYPAD_R3,
                Button::DPadUp => libretro_sys::DEVICE_ID_JOYPAD_UP,
                Button::DPadDown => libretro_sys::DEVICE_ID_JOYPAD_DOWN,
                Button::DPadLeft => libretro_sys::DEVICE_ID_JOYPAD_LEFT,
                Button::DPadRight => libretro_sys::DEVICE_ID_JOYPAD_RIGHT,
                Button::Unknown => continue,
            };

            if release {
                self.input_state &= !(1 << button);
            } else {
                self.input_state |= 1 << button;
            }
        }
    }

    fn input_state(&mut self, port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16 {
        if device != DEVICE_JOYPAD || port != 0 || index != 0 {
            return 0;
        }

        self.input_state & (1 << id)
    }

    fn can_dupe_frames(&mut self) -> bool {
        true
    }
}

enum Command {
    SaveState,
    LoadState,
    ToggleTurbo,
}

use std::ffi::c_uint;
use std::fs;
use std::path::PathBuf;

use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{io, thread, vec};

use anyhow::{anyhow, Context, Result};

use clap::Parser;

use enumset::EnumSet;
use gilrs::Gilrs;

use libretro_sys::PixelFormat;
use parking_lot::RwLock;
use rodio::Source;

use crate::audio::RetroAudio;
use crate::core::{Callbacks, Core};
use crate::video::Frame;

mod ap_remote;
mod audio;
pub(crate) mod core;
mod environment;
mod gui;
mod input;
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
) -> Result<(Receiver<Option<Frame>>, core::Handle)> {
    let core = core.into();
    let rom = rom.into();

    let (frame_tx, frame_rx) = sync_channel(1);
    let (audio_tx, audio_rx) = sync_channel(1);

    let core_host = core::Host::new();
    let core_handle = core_host.handle();

    thread::spawn(move || {
        let (_stream, stream_handle) = rodio::OutputStream::try_default()?;

        let gilrs = Gilrs::new()
            .map_err(|err| anyhow!("{err}"))
            .context("failed to initialize gilrs")?;

        for (id, gamepad) in gilrs.gamepads() {
            println!("Gamepad #{id}: {:?}", gamepad.name());
        }

        let sram_path = rom.with_extension("sram");

        let speed_factor = Arc::new(RwLock::new(1.0));

        let callbacks = ApeCallbacks {
            frame_tx,
            audio_tx,
            gilrs,
            egui_ctx,
            buttons: <_>::default(),
            speed_factor: Arc::clone(&speed_factor),
        };

        let core_config = core::Config {
            core,
            rom,
            callbacks: callbacks.boxed(),
        };

        let mut last_sram_save = Instant::now();

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

            ap_remote::start(core_host.handle());
            remote::start(core_host.handle());

            let system_av_info = core.get_system_av_info();

            println!("{:#?}", system_av_info);
            // panic!("sample rate: {}", system_av_info.timing.sample_rate);

            let retro_audio = RetroAudio {
                rx: audio_rx,
                current_frame: Vec::new().into_iter(),
                base_sample_rate: system_av_info.timing.sample_rate as f32,
                speed_factor: Arc::clone(&speed_factor),
            };

            thread::spawn(move || {
                let res = stream_handle
                    .play_raw(retro_audio.convert_samples())
                    .context("failed to play stream");

                if let Err(err) = res {
                    eprintln!("Error while playing audio: {err}");
                }
            });

            loop {
                core_host.run(core);

                if last_sram_save.elapsed() >= Duration::from_secs(5) {
                    if let Err(err) = core.save_sram_to(&sram_path) {
                        eprintln!("Failed to save SRAM: {err:?}");
                    }

                    last_sram_save = Instant::now();
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
    egui_ctx: egui::Context,
    buttons: EnumSet<input::Button>,
    speed_factor: Arc<RwLock<f32>>,
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

            if usize::from(event.id) != 0 {
                continue;
            }

            let button = match event.event {
                gilrs::EventType::ButtonPressed(button, _) => button,
                gilrs::EventType::ButtonReleased(button, _) => {
                    release = true;
                    button
                }
                _ => continue,
            };

            let Some(button) = input::Button::from_gilrs(button) else {
                continue;
            };

            match button {
                input::Button::Down => self.buttons -= input::Button::Up,
                input::Button::Up => self.buttons -= input::Button::Down,
                input::Button::Left => self.buttons -= input::Button::Right,
                input::Button::Right => self.buttons -= input::Button::Left,
                _ => {}
            };

            // TODO: move overrides to config
            let button = match button {
                input::Button::B => input::Button::A,
                input::Button::Y => input::Button::B,
                input::Button::L => input::Button::X,
                input::Button::A => continue,
                input::Button::X => continue,
                _ => button,
            };

            if button == input::Button::R {
                if release {
                    *self.speed_factor.write() = 1.;
                } else {
                    *self.speed_factor.write() = 2.;
                }

                continue;
            }

            if release {
                self.buttons.remove(button);
            } else {
                self.buttons.insert(button);
            }
        }
    }

    fn input_buttons(&self, port: c_uint) -> EnumSet<input::Button> {
        self.buttons
    }

    fn can_dupe_frames(&mut self) -> bool {
        true
    }
}

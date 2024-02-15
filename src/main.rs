use std::ffi::c_uint;
use std::path::{Path, PathBuf};

use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Mutex;
use std::time::Duration;
use std::{thread, vec};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use gilrs::{Button, Gilrs};

use libretro_sys::{PixelFormat, DEVICE_JOYPAD};
use minifb::{Key, Window, WindowOptions};
use rodio::Source;

use crate::audio::RetroAudio;
use crate::core::{Callbacks, Core};
use crate::video::Frame;

mod audio;
mod core;
mod environment;
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

    run(&cli.core, &cli.rom)?;

    Ok(())
}

fn run(core: impl AsRef<Path>, rom: impl AsRef<Path>) -> Result<()> {
    let (_stream, stream_handle) = rodio::OutputStream::try_default()?;

    let gilrs = Gilrs::new()
        .map_err(|err| anyhow!("{err}"))
        .context("failed to initialize gilrs")?;

    let (frame_tx, frame_rx) = sync_channel(1);
    let (audio_tx, audio_rx) = sync_channel(1);
    let (command_tx, command_rx) = sync_channel(32);

    let callbacks = ApeCallbacks {
        frame_tx,
        audio_tx,
        gilrs,
        input_state: 0,
        command_tx,
    };

    let core_config = core::Config {
        core: core.as_ref().to_owned(),
        rom: rom.as_ref().to_owned(),
        callbacks: callbacks.boxed(),
    };

    Core::load(core_config, |core| -> Result<()> {
        let system_av_info = core.get_system_av_info();

        println!("{:#?}", system_av_info);
        // panic!("sample rate: {}", system_av_info.timing.sample_rate);

        let retro_audio = RetroAudio {
            rx: audio_rx,
            current_frame: Vec::new().into_iter(),
            sample_rate: system_av_info.timing.sample_rate as u32,
        };

        thread::spawn(move || {
            let res = stream_handle
                .play_raw(retro_audio.convert_samples())
                .context("failed to play stream");

            if let Err(err) = res {
                eprintln!("Error while playing audio: {err}");
            }
        });

        let window_options = WindowOptions {
            resize: true,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..Default::default()
        };

        let mut saved_state = None;

        let scale = 3;
        let window_width = system_av_info.geometry.base_width as usize * scale;
        let window_height = system_av_info.geometry.base_height as usize * scale;
        let mut window = Window::new("APE", window_width, window_height, window_options)
            .context("failed to open window")?;

        window.limit_update_rate(Some(Duration::from_secs(1) / 61));

        let mut current_frame = Frame::empty();

        while window.is_open() && !window.is_key_down(Key::Escape) {
            core.run();

            if let Ok(frame) = frame_rx.recv_timeout(Duration::from_secs(1) / 60) {
                if let Some(frame) = frame {
                    current_frame = frame;
                }

                let buffer = current_frame.buffer_to_packed_argb32();

                window
                    .update_with_buffer(&buffer, current_frame.width, current_frame.height)
                    .context("failed to update window with buffer")?;
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
                }
            }
        }

        Ok(())
    })
    .context("failed to load core")?
    .context("runtime error")?;

    println!("Exiting normally");

    Ok(())
}

struct ApeCallbacks {
    frame_tx: SyncSender<Option<Frame>>,
    audio_tx: SyncSender<Vec<i16>>,
    gilrs: Gilrs,
    input_state: i16,
    command_tx: SyncSender<Command>,
}

impl Callbacks for ApeCallbacks {
    fn video_refresh(&mut self, frame: Option<Frame>) {
        if self.frame_tx.try_send(frame).is_err() {
            eprintln!("Dropping frame, failed to send");
        }
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

            eprintln!("Pressed button {button:?}");

            let button = match button {
                Button::South => libretro_sys::DEVICE_ID_JOYPAD_B,
                Button::East => libretro_sys::DEVICE_ID_JOYPAD_A,
                Button::North => libretro_sys::DEVICE_ID_JOYPAD_X,
                Button::West => libretro_sys::DEVICE_ID_JOYPAD_Y,
                Button::C => continue,
                Button::Z => continue,
                Button::LeftTrigger => {
                    // libretro_sys::DEVICE_ID_JOYPAD_L
                    self.command_tx.try_send(Command::LoadState).ok();
                    continue;
                }
                Button::LeftTrigger2 => libretro_sys::DEVICE_ID_JOYPAD_L2,
                Button::RightTrigger => {
                    // libretro_sys::DEVICE_ID_JOYPAD_R
                    self.command_tx.try_send(Command::SaveState).ok();
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
}

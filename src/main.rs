use core::slice;
use std::ffi::{c_uint, c_void, CStr};
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::ptr::null;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::time::Duration;
use std::{fs, iter, mem, thread, vec};

use anyhow::{bail, Context, Result};
use clap::Parser;
use libloading::Library;
use libretro_sys::{
    CoreAPI, GameGeometry, GameInfo, PixelFormat, SystemAvInfo, SystemTiming, Variable,
};
use minifb::{Key, Window, WindowOptions};
use rodio::Source;

use crate::environment::Environment;
use crate::environment_command::EnvironmentCommand;

mod environment;
mod environment_command;

const EXPECTED_LIB_RETRO_VERSION: u32 = 1;
const WIDTH: usize = 160 * 4;
const HEIGHT: usize = 144 * 4;

static ENVIRONMENT: Mutex<Option<Environment>> = Mutex::new(None);

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

    let (core_api, frame_rx, audio_rx) =
        unsafe { load_core(core, rom).context("failed to load core")? };

    struct RetroAudio {
        rx: Receiver<Vec<i16>>,
        current_frame: vec::IntoIter<i16>,
        sample_rate: u32,
    }

    impl rodio::Source for RetroAudio {
        fn current_frame_len(&self) -> Option<usize> {
            None
        }

        fn channels(&self) -> u16 {
            2
        }

        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    let mut system_av_info = SystemAvInfo {
        geometry: GameGeometry {
            aspect_ratio: f32::NAN,
            base_width: 0,
            base_height: 0,
            max_width: 0,
            max_height: 0,
        },
        timing: SystemTiming {
            fps: 0.,
            sample_rate: 0.,
        },
    };

    unsafe {
        (core_api.retro_get_system_av_info)(&mut system_av_info);
    }

    println!("{:#?}", system_av_info);
    // panic!("sample rate: {}", system_av_info.timing.sample_rate);

    let retro_audio = RetroAudio {
        rx: audio_rx,
        current_frame: Vec::new().into_iter(),
        sample_rate: system_av_info.timing.sample_rate as u32,
    };

    thread::spawn(move || {
        stream_handle
            .play_raw(retro_audio.convert_samples())
            .context("failed to play stream")
            .unwrap();
    });

    println!("POST");

    impl Iterator for RetroAudio {
        type Item = i16;

        fn next(&mut self) -> Option<Self::Item> {
            match self.current_frame.next() {
                Some(sample) => Some(sample),
                None => {
                    self.current_frame = self.rx.recv().unwrap().into_iter();
                    self.current_frame.next()
                }
            }
        }
    }

    let mut window = Window::new("APE", WIDTH, HEIGHT, WindowOptions::default())
        .context("failed to open window")?;

    window.limit_update_rate(Some(Duration::from_secs(1) / 61));

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // for key in window.get_keys() {
        //     match key {
        //         Key::Left if x > 0 => x -= 1,
        //         Key::Right if x < WIDTH - 1 => x += 1,
        //         Key::Up if y > 0 => y -= 1,
        //         Key::Down if y < HEIGHT - 1 => y += 1,
        //         _ => {}
        //     }
        // }

        unsafe { (core_api.retro_run)() };

        if let Ok(frame) = frame_rx.recv_timeout(Duration::from_secs(1) / 60) {
            // eprintln!("Updating window with buffer");
            let buffer = frame
                .buffer
                .chunks_exact(4)
                .map(|chunk| {
                    (chunk[3] as u32) << 24
                        | (chunk[2] as u32) << 16
                        | (chunk[1] as u32) << 8
                        | (chunk[0] as u32)
                })
                .collect::<Vec<u32>>();

            window
                .update_with_buffer(&buffer, frame.width, frame.height)
                .context("failed to update window with buffer")?;
        }

        // window.update()
    }

    Ok(())
}

unsafe fn load_core(
    path: impl AsRef<Path>,
    rom: impl AsRef<Path>,
) -> Result<(CoreAPI, Receiver<Frame>, Receiver<Vec<i16>>)> {
    let (env, frame_rx, audio_rx) = Environment::new();

    env.register()?;

    unsafe fn load<T: Copy>(library: &Library, symbol: &str) -> Result<T> {
        let item = library
            .get::<T>(symbol.as_bytes())
            .with_context(|| format!("failed to load symbol `{}` from core", symbol))?;

        Ok(*item)
    }

    let lib = Library::new(path.as_ref()).context("failed to load core")?;
    // TODO: manage library lifetime
    let lib = Box::leak(Box::new(lib));
    let core_api = CoreAPI {
        retro_set_environment: load(&lib, "retro_set_environment")?,
        retro_set_video_refresh: load(&lib, "retro_set_video_refresh")?,
        retro_set_audio_sample: load(&lib, "retro_set_audio_sample")?,
        retro_set_audio_sample_batch: load(&lib, "retro_set_audio_sample_batch")?,
        retro_set_input_poll: load(&lib, "retro_set_input_poll")?,
        retro_set_input_state: load(&lib, "retro_set_input_state")?,

        retro_init: load(&lib, "retro_init")?,
        retro_deinit: load(&lib, "retro_deinit")?,

        retro_api_version: load(&lib, "retro_api_version")?,

        retro_get_system_info: load(&lib, "retro_get_system_info")?,
        retro_get_system_av_info: load(&lib, "retro_get_system_av_info")?,
        retro_set_controller_port_device: load(&lib, "retro_set_controller_port_device")?,

        retro_reset: load(&lib, "retro_reset")?,
        retro_run: load(&lib, "retro_run")?,

        retro_serialize_size: load(&lib, "retro_serialize_size")?,
        retro_serialize: load(&lib, "retro_serialize")?,
        retro_unserialize: load(&lib, "retro_unserialize")?,

        retro_cheat_reset: load(&lib, "retro_cheat_reset")?,
        retro_cheat_set: load(&lib, "retro_cheat_set")?,

        retro_load_game: load(&lib, "retro_load_game")?,
        retro_load_game_special: load(&lib, "retro_load_game_special")?,
        retro_unload_game: load(&lib, "retro_unload_game")?,

        retro_get_region: load(&lib, "retro_get_region")?,
        retro_get_memory_data: load(&lib, "retro_get_memory_data")?,
        retro_get_memory_size: load(&lib, "retro_get_memory_size")?,
    };

    // The following libretro calls must not be reordered.
    // > Implementations are designed to be single-instance, so global state is allowed.
    // > Should the frontend call these functions in wrong order, undefined behavior occurs.
    // https://docs.libretro.com/development/cores/developing-cores/#implementing-the-api
    let api_version = (core_api.retro_api_version)();

    println!("Core API version: {}", api_version);

    if api_version != EXPECTED_LIB_RETRO_VERSION {
        bail!(
            "Core was compiled against libretro version `{api_version}`, \
                but expected version `{EXPECTED_LIB_RETRO_VERSION}`",
        );
    }

    (core_api.retro_set_environment)(environment_cb);

    (core_api.retro_set_video_refresh)(video_refresh_cb);
    (core_api.retro_set_audio_sample)(audio_sample_cb);
    (core_api.retro_set_audio_sample_batch)(audio_sample_batch_cb);
    (core_api.retro_set_input_poll)(input_poll_cb);
    (core_api.retro_set_input_state)(input_state_cb);

    (core_api.retro_init)();

    {
        let rom = fs::read(rom).context("Failed to read rom")?;
        let game_info = GameInfo {
            path: null(),
            data: rom.as_ptr().cast(),
            size: rom.len(),
            meta: null(),
        };

        let load_game_successful = (core_api.retro_load_game)(&game_info);

        if !load_game_successful {
            bail!("Failed to load game");
        }
    }

    // mem::forget(lib);

    Ok((core_api, frame_rx, audio_rx))
}

unsafe extern "C" fn environment_cb(command: u32, data: *mut c_void) -> bool {
    let mut env = match ENVIRONMENT.try_lock() {
        Ok(env) => env,
        Err(err) => {
            eprintln!("BUG: failed to lock env: {err}");
            return false;
        }
    };

    let Some(env) = &mut *env else {
        eprintln!("BUG: environment cb called without an existing env");
        return false;
    };

    let Some(command) = EnvironmentCommand::from_repr(command) else {
        eprintln!("Unknown retro_set_environment command `{command}`");
        return false;
    };

    match command {
        EnvironmentCommand::SET_PIXEL_FORMAT => {
            let pixel_format = *data.cast_const().cast::<c_uint>();
            let Some(pixel_format) = PixelFormat::from_uint(pixel_format) else {
                eprintln!("Unknown pixel format variant `{pixel_format}`");
                return false;
            };

            env.set_pixel_format(pixel_format)
        }
        EnvironmentCommand::SET_VARIABLES => {
            let mut variables = data.cast_const().cast::<Variable>();
            let variables = iter::from_fn(|| {
                let variable = variables.as_ref()?;

                // Safety: `.as_ref()?` guarantees non-null ptr
                let key = CStr::from_ptr(variable.key.as_ref()?);
                let key = key.to_string_lossy();

                // Safety: `.as_ref()?` guarantees non-null ptr
                let value = CStr::from_ptr(variable.value.as_ref()?);
                let value = value.to_string_lossy();

                // Safety: valid until either `key` or `value` are null
                variables = variables.add(1);

                Some((key, value))
            })
            // Safety: fusing prevents iterating past sentinel variable
            .fuse();

            env.set_variables(variables)
        }
        EnvironmentCommand::GET_VARIABLE => {
            let Some(variable) = data.cast::<Variable>().as_mut() else {
                eprintln!("get_variable called with null variable");
                return false;
            };

            let Some(key) = variable.key.as_ref() else {
                eprintln!("get_variable called with null key");
                return false;
            };
            let key = CStr::from_ptr(key).to_string_lossy();

            variable.value = match env.get_variable(&key) {
                Some(value) => {
                    eprintln!("returning get_variable for key {key}");
                    value.as_ptr()
                }
                None => {
                    eprintln!("get_variable called with unknown key");
                    null()
                }
            };

            true
        }
        _ => {
            // eprintln!("Unhandled retro_set_environment command `{command:?}`");
            false
        }
    }
}

unsafe extern "C" fn video_refresh_cb(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    // eprintln!("In video refresh cb!");

    if data.is_null() {
        return;
    }

    let mut env = match ENVIRONMENT.try_lock() {
        Ok(env) => env,
        Err(err) => {
            eprintln!("BUG: failed to lock env: {err}");
            return;
        }
    };

    let Some(env) = &mut *env else {
        eprintln!("BUG: video_refresh cb called without an existing env");
        return;
    };

    if *env.pixel_format() != PixelFormat::ARGB8888 {
        eprintln!("Unimplemented pixel format {:?}", env.pixel_format());
        return;
    }

    if pitch as u32 != (4 * width) {
        eprintln!("Unsupported pitch `{pitch}` (width = {width}, height = {height})");
        return;
    }

    let buffer = slice::from_raw_parts(data.cast::<u8>(), (4 * width * height) as usize).to_owned();
    let frame = Frame {
        buffer,
        width: width as usize,
        height: height as usize,
        pitch,
    };

    env.send_frame(frame);
}

unsafe extern "C" fn audio_sample_cb(left: i16, right: i16) {
    eprintln!("BAD: In audio sample cb!");
}

struct Frame {
    pub buffer: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}

unsafe extern "C" fn audio_sample_batch_cb(data: *const i16, frames: usize) -> usize {
    let mut env = match ENVIRONMENT.try_lock() {
        Ok(env) => env,
        Err(err) => {
            eprintln!("BUG: failed to lock env: {err}");
            return 1;
        }
    };

    let Some(env) = &mut *env else {
        eprintln!("BUG: video_refresh cb called without an existing env");
        return 1;
    };

    // println!("in audio sample batch cb ({frames} frames)");

    let sample = slice::from_raw_parts(data, frames * 2).to_vec();

    // println!("{sample:#?}");

    env.send_audio(sample);

    frames
}

unsafe extern "C" fn input_poll_cb() {
    // eprintln!("In input poll cb!");
}

unsafe extern "C" fn input_state_cb(
    port: c_uint,
    device: c_uint,
    index: c_uint,
    id: c_uint,
) -> i16 {
    // eprintln!("In input state cb!");

    0
}

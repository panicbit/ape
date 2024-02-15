use std::fs;
use std::os::raw::c_void;
use std::path::Path;
use std::path::PathBuf;
use std::ptr::null;

use anyhow::Context;
use anyhow::{bail, Result};
use libretro_sys::GameGeometry;
use libretro_sys::GameInfo;
use libretro_sys::SystemAvInfo;
use libretro_sys::SystemTiming;

use self::api::Api;
pub use self::callbacks::*;
pub use self::state::*;

mod api;
mod callbacks;
mod state;

const EXPECTED_LIB_RETRO_VERSION: u32 = 1;

pub struct Core {
    api: Api,
}

impl Core {
    pub fn load<F, R>(config: Config, f: F) -> Result<R>
    where
        F: FnOnce(&mut Core) -> R,
    {
        unsafe {
            let is_core_loaded = STATE.with_borrow_mut(|state| state.is_core_loaded);

            if is_core_loaded {
                bail!("only one core per thread allowed")
            }

            // TODO: prevent the same core from being loaded more than once in the same process

            let api = Api::load(config.core)?;
            let mut core = Core { api };

            core.check_api_version_match()?;
            core.register_callbacks(config.callbacks);
            (core.api.retro_init)();

            if let Err(err) = core.load_game(&config.rom) {
                (core.api.retro_deinit)();

                return Err(err.context("failed to load game"));
            }

            let res = f(&mut core);

            (core.api.retro_unload_game)();
            (core.api.retro_deinit)();

            callbacks::drop();

            STATE.set(State::new());

            Ok(res)
        }
    }

    pub fn get_system_av_info(&self) -> SystemAvInfo {
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
            (self.api.retro_get_system_av_info)(&mut system_av_info);
        }

        system_av_info
    }

    pub fn run(&mut self) {
        unsafe { (self.api.retro_run)() }
    }

    pub fn state(&mut self) -> Result<Vec<u8>> {
        unsafe {
            let size = (self.api.retro_serialize_size)();
            let mut state = Vec::<u8>::with_capacity(size);

            let success = (self.api.retro_serialize)(state.as_mut_ptr().cast::<c_void>(), size);

            if !success {
                bail!("state serialization failed");
            }

            state.set_len(size);

            Ok(state)
        }
    }

    pub fn restore_state(&mut self, state: &[u8]) -> Result<()> {
        unsafe {
            let success =
                (self.api.retro_unserialize)(state.as_ptr().cast::<c_void>(), state.len());

            if !success {
                bail!("failed to restore state");
            }

            Ok(())
        }
    }
}

impl Core {
    unsafe fn check_api_version_match(&mut self) -> Result<()> {
        let api_version = (self.api.retro_api_version)();

        if api_version != EXPECTED_LIB_RETRO_VERSION {
            bail!(
                "Core was compiled against libretro version `{api_version}`, \
            but expected version `{EXPECTED_LIB_RETRO_VERSION}`",
            );
        }

        Ok(())
    }

    unsafe fn register_callbacks(&mut self, callbacks: Box<dyn Callbacks>) {
        callbacks::register(callbacks);

        (self.api.retro_set_environment)(callbacks::ffi::environment);
        (self.api.retro_set_video_refresh)(callbacks::ffi::video_refresh);
        (self.api.retro_set_audio_sample)(callbacks::ffi::audio_sample);
        (self.api.retro_set_audio_sample_batch)(callbacks::ffi::audio_sample_batch);
        (self.api.retro_set_input_poll)(callbacks::ffi::input_poll);
        (self.api.retro_set_input_state)(callbacks::ffi::input_state);
    }

    unsafe fn load_game(&mut self, rom: impl AsRef<Path>) -> Result<()> {
        let rom = fs::read(rom).context("Failed to read rom")?;

        // TODO: ask core whether to provide path or data
        let game_info = GameInfo {
            path: null(),
            data: rom.as_ptr().cast(),
            size: rom.len(),
            meta: null(),
        };

        let load_game_successful = (self.api.retro_load_game)(&game_info);

        if !load_game_successful {
            bail!("Failed to load game");
        }

        Ok(())
    }
}

pub struct Config {
    pub core: PathBuf,
    pub rom: PathBuf,
    pub callbacks: Box<dyn Callbacks>,
}

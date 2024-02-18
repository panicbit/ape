use std::ffi::c_void;
use std::ops::Deref;
use std::path::Path;
use std::ptr;

use anyhow::{Context, Result};
use libloading::Library;
use libretro_sys::CoreAPI;

pub(super) struct Api {
    _library: Library,
    core_api: CoreAPI,
    _opt_out_of_send_sync: *const (),
    pub retro_serialize: unsafe extern "C" fn(data: *mut c_void, size: usize) -> bool,
}

impl Api {}

impl Api {
    pub unsafe fn load(path: impl AsRef<Path>) -> Result<Self> {
        let library = Library::new(path.as_ref()).context("failed to load core library")?;
        let core_api = CoreAPI {
            retro_set_environment: deref_symbol(&library, "retro_set_environment")?,
            retro_set_video_refresh: deref_symbol(&library, "retro_set_video_refresh")?,
            retro_set_audio_sample: deref_symbol(&library, "retro_set_audio_sample")?,
            retro_set_audio_sample_batch: deref_symbol(&library, "retro_set_audio_sample_batch")?,
            retro_set_input_poll: deref_symbol(&library, "retro_set_input_poll")?,
            retro_set_input_state: deref_symbol(&library, "retro_set_input_state")?,

            retro_init: deref_symbol(&library, "retro_init")?,
            retro_deinit: deref_symbol(&library, "retro_deinit")?,

            retro_api_version: deref_symbol(&library, "retro_api_version")?,

            retro_get_system_info: deref_symbol(&library, "retro_get_system_info")?,
            retro_get_system_av_info: deref_symbol(&library, "retro_get_system_av_info")?,
            retro_set_controller_port_device: deref_symbol(
                &library,
                "retro_set_controller_port_device",
            )?,

            retro_reset: deref_symbol(&library, "retro_reset")?,
            retro_run: deref_symbol(&library, "retro_run")?,

            retro_serialize_size: deref_symbol(&library, "retro_serialize_size")?,
            retro_serialize: deref_symbol(&library, "retro_serialize")?,
            retro_unserialize: deref_symbol(&library, "retro_unserialize")?,

            retro_cheat_reset: deref_symbol(&library, "retro_cheat_reset")?,
            retro_cheat_set: deref_symbol(&library, "retro_cheat_set")?,

            retro_load_game: deref_symbol(&library, "retro_load_game")?,
            retro_load_game_special: deref_symbol(&library, "retro_load_game_special")?,
            retro_unload_game: deref_symbol(&library, "retro_unload_game")?,

            retro_get_region: deref_symbol(&library, "retro_get_region")?,
            retro_get_memory_data: deref_symbol(&library, "retro_get_memory_data")?,
            retro_get_memory_size: deref_symbol(&library, "retro_get_memory_size")?,
        };

        let retro_serialize = deref_symbol(&library, "retro_serialize")?;

        Ok(Self {
            _library: library,
            core_api,
            _opt_out_of_send_sync: ptr::null(),
            retro_serialize,
        })
    }
}

unsafe fn deref_symbol<T: Copy>(library: &Library, symbol: &str) -> Result<T> {
    let item = library
        .get::<T>(symbol.as_bytes())
        .with_context(|| format!("failed to load symbol `{}` from core", symbol))?;

    Ok(*item)
}

impl Deref for Api {
    type Target = CoreAPI;

    fn deref(&self) -> &Self::Target {
        &self.core_api
    }
}

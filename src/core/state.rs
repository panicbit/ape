use std::cell::RefCell;

use libretro_sys::PixelFormat;

use crate::core::MemoryMap;

thread_local! {
    pub static STATE: RefCell<State> = RefCell::new(State::new());
}

pub struct State {
    pub is_core_loaded: bool,
    pub pixel_format: PixelFormat,
    pub memory_map: MemoryMap,
}

impl State {
    pub fn new() -> Self {
        Self {
            is_core_loaded: false,
            pixel_format: PixelFormat::ARGB1555,
            memory_map: MemoryMap::empty(),
        }
    }
}

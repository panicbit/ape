use std::cell::RefCell;

use libretro_sys::PixelFormat;

thread_local! {
    pub static STATE: RefCell<State> = RefCell::new(State::new());
}

pub struct State {
    pub is_core_loaded: bool,
    pub pixel_format: PixelFormat,
}

impl State {
    pub fn new() -> Self {
        Self {
            is_core_loaded: false,
            pixel_format: PixelFormat::ARGB1555,
        }
    }
}

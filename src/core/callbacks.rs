use std::cell::RefCell;
use std::ffi::c_uint;

use enumset::EnumSet;
use libretro_sys::PixelFormat;

use crate::input;
use crate::video::Frame;

pub mod ffi;

thread_local! {
    pub static CALLBACKS: RefCell<Box<dyn Callbacks>> = RefCell::new(Stub.boxed());
}

pub fn register(callbacks: Box<dyn Callbacks>) {
    CALLBACKS.set(callbacks);
}

pub fn drop() {
    CALLBACKS.set(Stub.boxed());
}

pub trait Callbacks {
    fn video_refresh(&mut self, frame: Option<Frame>);
    fn supports_pixel_format(&mut self, pixel_format: PixelFormat) -> bool;
    fn audio_sample(&mut self, left: i16, right: i16);
    fn audio_samples(&mut self, samples: &[i16]);
    fn input_poll(&mut self);
    fn input_buttons(&self, port: c_uint) -> EnumSet<input::Button>;
    fn can_dupe_frames(&mut self) -> bool {
        false
    }

    fn boxed(self) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

pub struct Stub;

impl Callbacks for Stub {
    fn video_refresh(&mut self, _frame: Option<Frame>) {
        eprintln!("WARNING: video_refresh is stubbed");
    }

    fn supports_pixel_format(&mut self, _pixel_format: PixelFormat) -> bool {
        eprintln!("WARNING: supports_pixel_format is stubbed");

        false
    }

    fn audio_sample(&mut self, _left: i16, _right: i16) {
        eprintln!("WARNING: audio_sample is stubbed");
    }

    fn audio_samples(&mut self, _samples: &[i16]) {
        eprintln!("WARNING: audio_samples is stubbed");
    }

    fn input_poll(&mut self) {
        eprintln!("WARNING: input_poll is stubbed");
    }

    fn input_buttons(&self, _port: c_uint) -> EnumSet<input::Button> {
        eprintln!("WARNING: input_buttons is stubbed");
        EnumSet::empty()
    }
}

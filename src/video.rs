use std::ffi::{c_uint, c_void};
use std::slice;

use itertools::Itertools;
use libretro_sys::PixelFormat;

pub struct Frame {
    pub buffer: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub pixel_format: PixelFormat,
}

impl Frame {
    pub unsafe fn from_raw(
        data: *const c_void,
        width: c_uint,
        height: c_uint,
        pitch: usize,
        pixel_format: PixelFormat,
    ) -> Self {
        let width = width as usize;
        let height = height as usize;
        let size = height * pitch;
        let buffer = slice::from_raw_parts(data.cast::<u8>(), size).to_owned();

        Self {
            buffer,
            width,
            height,
            pitch,
            pixel_format,
        }
    }

    pub fn buffer_to_packed_argb32(&self) -> Vec<u32> {
        match self.pixel_format {
            PixelFormat::ARGB1555 => todo!(),
            PixelFormat::ARGB8888 => self.argb8888_buffer_to_packed_argb32(),
            PixelFormat::RGB565 => todo!(),
        }
    }

    fn argb8888_buffer_to_packed_argb32(&self) -> Vec<u32> {
        let bytes_per_pixel = 4;
        let bytes_per_row = bytes_per_pixel * self.width;

        self.buffer
            .chunks_exact(self.pitch)
            .flat_map(|row| &row[..bytes_per_row])
            .copied()
            .tuples()
            .map(|(b, g, r, a)| (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | (b as u32))
            .collect_vec()
    }
}

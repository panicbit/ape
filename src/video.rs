use std::ffi::{c_uint, c_void};
use std::slice;

use itertools::Itertools;
use libretro_sys::PixelFormat;

pub type R8 = u8;
pub type G8 = u8;
pub type B8 = u8;
pub type A8 = u8;

pub struct Frame {
    pub buffer: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub pixel_format: PixelFormat,
}

impl Frame {
    pub fn empty() -> Self {
        Self {
            buffer: Vec::new(),
            width: 0,
            height: 0,
            pitch: 0,
            pixel_format: PixelFormat::ARGB8888,
        }
    }

    pub unsafe fn from_raw(
        data: *const c_void,
        width: c_uint,
        height: c_uint,
        pitch: usize,
        pixel_format: PixelFormat,
    ) -> Option<Self> {
        if data.is_null() {
            return None;
        }

        let width = width as usize;
        let height = height as usize;
        let size = height * pitch;
        let buffer = slice::from_raw_parts(data.cast::<u8>(), size).to_vec();

        Some(Self {
            buffer,
            width,
            height,
            pitch,
            pixel_format,
        })
    }

    pub fn buffer_to_packed_rgb888(&self) -> Vec<u8> {
        let len = self.width * self.height * 3;
        let mut pixels = Vec::with_capacity(len);

        self.for_each_pixel(|r, g, b, _a| {
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
        });

        pixels
    }

    pub fn buffer_to_packed_argb32(&self) -> Vec<u32> {
        let len = self.width * self.height * 4;
        let mut pixels = Vec::with_capacity(len);

        self.for_each_pixel(|r, g, b, a| {
            let pixel = u32::from_be_bytes([a, r, g, b]);

            pixels.push(pixel);
        });

        pixels
    }

    pub fn for_each_pixel(&self, f: impl FnMut(R8, G8, B8, A8)) {
        match self.pixel_format {
            PixelFormat::ARGB1555 => todo!(),
            PixelFormat::ARGB8888 => self.for_each_pixel_argb8888(f),
            PixelFormat::RGB565 => self.for_each_pixel_rgb565(f),
        }
    }

    fn for_each_pixel_argb8888(&self, mut f: impl FnMut(R8, G8, B8, A8)) {
        let bytes_per_pixel = 4;
        let bytes_per_row = bytes_per_pixel * self.width;

        self.buffer
            .chunks_exact(self.pitch)
            .flat_map(|row| &row[..bytes_per_row])
            .copied()
            .tuples()
            .for_each(|(b1, b2, b3, b4)| {
                let pixel = u32::from_ne_bytes([b1, b2, b3, b4]);
                let [a, r, g, b] = pixel.to_be_bytes();

                f(r, g, b, a);
            })
    }

    fn for_each_pixel_rgb565(&self, mut f: impl FnMut(R8, G8, B8, A8)) {
        let bytes_per_pixel = 2;
        let bytes_per_row = bytes_per_pixel * self.width;
        let max_r = (2u8.pow(5) - 1) as f32;
        let max_g = (2u8.pow(6) - 1) as f32;
        let max_b = (2u8.pow(5) - 1) as f32;

        self.buffer
            .chunks_exact(self.pitch)
            .flat_map(|row| &row[..bytes_per_row])
            .copied()
            .tuples()
            .for_each(|(b1, b2)| {
                let pixel = u16::from_ne_bytes([b1, b2]);
                let r = pixel >> 11;
                let r = ((r as f32 / max_r) * 255.).round() as u8;
                let g = (pixel >> 5) & 0b111111;
                let g = ((g as f32 / max_g) * 255.).round() as u8;
                let b = pixel & 0b11111;
                let b = ((b as f32 / max_b) * 255.).round() as u8;
                let a = 0;

                f(r, g, b, a)
            })
    }
}

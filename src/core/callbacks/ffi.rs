use std::borrow::{Borrow, BorrowMut};
use std::ffi::{c_uint, c_void};
use std::slice;

use libretro_sys::PixelFormat;

use crate::core::{CALLBACKS, STATE};
use crate::environment::Command;
use crate::video::Frame;

pub unsafe extern "C" fn video_refresh(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    let pixel_format = STATE.with_borrow(|state| state.pixel_format);
    let frame = Frame::from_raw(data, width, height, pitch, pixel_format);

    CALLBACKS.with_borrow_mut(|callbacks| callbacks.video_refresh(frame));
}

pub unsafe extern "C" fn audio_sample(left: i16, right: i16) {
    CALLBACKS.with_borrow_mut(|callbacks| callbacks.audio_sample(left, right))
}

pub unsafe extern "C" fn audio_sample_batch(samples: *const i16, num_frames: usize) -> usize {
    let num_channels = 2;
    let samples = slice::from_raw_parts(samples, num_channels * num_frames);

    CALLBACKS.with_borrow_mut(|callbacks| callbacks.audio_samples(samples));

    // TODO: allow high level API to control how many frames to consume
    num_frames
}

pub unsafe extern "C" fn input_poll() {
    CALLBACKS.with_borrow_mut(|callbacks| callbacks.input_poll());
}

pub unsafe extern "C" fn input_state(
    port: c_uint,
    device: c_uint,
    index: c_uint,
    id: c_uint,
) -> i16 {
    CALLBACKS.with_borrow_mut(|callbacks| callbacks.input_state(port, device, index, id))
}

pub unsafe extern "C" fn environment(command: u32, data: *mut c_void) -> bool {
    let Some(command) = Command::from_repr(command) else {
        eprintln!("Unknown retro_set_environment command `{command}`");
        return false;
    };

    match command {
        Command::SET_PIXEL_FORMAT => {
            let pixel_format = *data.cast_const().cast::<c_uint>();
            let Some(pixel_format) = PixelFormat::from_uint(pixel_format) else {
                eprintln!("Unknown pixel format variant `{pixel_format}`");
                return false;
            };

            let supported = CALLBACKS
                .with_borrow_mut(|callbacks| callbacks.supports_pixel_format(pixel_format));

            if supported {
                STATE.with_borrow_mut(|state| state.pixel_format = pixel_format);
            }

            supported
        }
        Command::GET_CAN_DUPE => {
            if !data.is_null() {
                let can_dupe = CALLBACKS.with_borrow_mut(|callbacks| callbacks.can_dupe_frames());

                *data.cast::<bool>() = can_dupe;
            }

            true
        }
        // Command::SET_VARIABLES => {
        //     let mut variables = data.cast_const().cast::<libretro_sys::Variable>();
        //     let variables = iter::from_fn(|| {
        //         let variable = variables.as_ref()?;

        //         // Safety: `.as_ref()?` guarantees non-null ptr
        //         let key = CStr::from_ptr(variable.key.as_ref()?);
        //         let key = key.to_string_lossy();

        //         // Safety: `.as_ref()?` guarantees non-null ptr
        //         let value = CStr::from_ptr(variable.value.as_ref()?);
        //         let value = value.to_string_lossy();

        //         // Safety: valid until either `key` or `value` are null
        //         variables = variables.add(1);

        //         Some((key, value))
        //     })
        //     // Safety: fusing prevents iterating past sentinel variable
        //     .fuse();

        //     env.set_variables(variables)
        // }
        // Command::GET_VARIABLE => {
        //     let Some(variable) = data.cast::<libretro_sys::Variable>().as_mut() else {
        //         eprintln!("get_variable called with null variable");
        //         return false;
        //     };

        //     let Some(key) = variable.key.as_ref() else {
        //         eprintln!("get_variable called with null key");
        //         return false;
        //     };
        //     let key = CStr::from_ptr(key).to_string_lossy();

        //     variable.value = match env.get_variable(&key) {
        //         Some(value) => {
        //             eprintln!("returning get_variable for key {key}");
        //             value.as_ptr()
        //         }
        //         None => {
        //             eprintln!("get_variable called with unknown key");
        //             null()
        //         }
        //     };

        //     true
        // }
        _ => {
            // eprintln!("Unhandled retro_set_environment command `{command:?}`");
            false
        }
    }
}

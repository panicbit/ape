use std::borrow::Cow;
use std::ffi::{c_uint, c_void, CStr, CString};
use std::iter;
use std::ptr::null;
use std::sync::mpsc::{self, Receiver, SyncSender};

use anyhow::{anyhow, Context, Result};
use gilrs::Gilrs;
use indexmap::IndexMap;
use libretro_sys::{
    PixelFormat, DEVICE_ID_JOYPAD_A, DEVICE_ID_JOYPAD_B, DEVICE_ID_JOYPAD_DOWN, DEVICE_ID_JOYPAD_L,
    DEVICE_ID_JOYPAD_L2, DEVICE_ID_JOYPAD_L3, DEVICE_ID_JOYPAD_LEFT, DEVICE_ID_JOYPAD_R,
    DEVICE_ID_JOYPAD_R2, DEVICE_ID_JOYPAD_R3, DEVICE_ID_JOYPAD_RIGHT, DEVICE_ID_JOYPAD_SELECT,
    DEVICE_ID_JOYPAD_START, DEVICE_ID_JOYPAD_UP, DEVICE_ID_JOYPAD_X, DEVICE_ID_JOYPAD_Y,
};

use crate::{Frame, ENVIRONMENT};
use command::Command;

mod command;

pub struct Environment {
    variables: IndexMap<String, Variable>,
    pixel_format: PixelFormat,
    frame_tx: SyncSender<Option<Frame>>,
    audio_tx: SyncSender<Vec<i16>>,
    gilrs: Gilrs,
    input_state: i16,
}

impl Environment {
    pub fn new(gilrs: Gilrs) -> (Self, Receiver<Option<Frame>>, Receiver<Vec<i16>>) {
        let (frame_tx, frame_rx) = mpsc::sync_channel(1);
        let (audio_tx, audio_rx) = mpsc::sync_channel(1);

        let this = Self {
            pixel_format: PixelFormat::ARGB1555,
            variables: IndexMap::new(),
            frame_tx,
            audio_tx,
            gilrs,
            input_state: 0,
        };

        (this, frame_rx, audio_rx)
    }

    pub fn pixel_format(&self) -> &PixelFormat {
        &self.pixel_format
    }

    pub fn set_pixel_format(&mut self, pixel_format: PixelFormat) -> bool {
        match pixel_format {
            PixelFormat::ARGB8888 => {
                eprintln!("Using pixel format `ARGB8888`");
                self.pixel_format = pixel_format;
                true
            }
            PixelFormat::RGB565 => {
                eprintln!("Using pixel format `RGB565`");
                self.pixel_format = pixel_format;
                true
            }
            _ => {
                eprintln!("Core requested unsupported pixel format `{pixel_format:?}`");
                false
            }
        }
    }

    pub fn set_variables<'k, 'v>(
        &mut self,
        variables: impl IntoIterator<Item = (Cow<'k, str>, Cow<'v, str>)>,
    ) -> bool {
        for (key, value) in variables {
            if !self.set_variable(key, value) {
                return false;
            }
        }

        true
    }

    pub fn set_variable(&mut self, key: Cow<str>, value: Cow<str>) -> bool {
        let variable = match Variable::parse(&value) {
            Ok(variable) => variable,
            Err(err) => {
                eprintln!("Failed to variable `{key}` = `{value}`: {err}");

                return false;
            }
        };

        eprintln!("Setting variable: {key} = {variable:#?}");

        self.variables.insert(key.into_owned(), variable);

        true
    }

    pub fn get_variable(&self, key: &str) -> Option<&CString> {
        self.variables.get(key).map(|var| &var.value)
    }

    pub fn register(self) -> Result<()> {
        let mut env = super::ENVIRONMENT
            .lock()
            .map_err(|err| anyhow!("BUG: failed to lock env while registering new env: {err}"))?;

        *env = Some(self);

        Ok(())
    }

    pub fn unregister() -> Result<()> {
        let mut env = super::ENVIRONMENT
            .lock()
            .map_err(|err| anyhow!("BUG: failed to lock env while unregistering env: {err}"))?;

        *env = None;

        Ok(())
    }

    pub(crate) fn send_frame(&self, frame: impl Into<Option<Frame>>) {
        if self.frame_tx.try_send(frame.into()).is_err() {
            eprintln!("Dropping frame, failed to send");
        }
    }

    pub(crate) fn send_audio(&self, sample: Vec<i16>) {
        self.audio_tx.send(sample).ok();
    }

    pub(crate) fn poll_input(&mut self) {
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
                gilrs::Button::South => DEVICE_ID_JOYPAD_B,
                gilrs::Button::East => DEVICE_ID_JOYPAD_A,
                gilrs::Button::North => DEVICE_ID_JOYPAD_X,
                gilrs::Button::West => DEVICE_ID_JOYPAD_Y,
                gilrs::Button::C => 0,
                gilrs::Button::Z => 0,
                gilrs::Button::LeftTrigger => DEVICE_ID_JOYPAD_L,
                gilrs::Button::LeftTrigger2 => DEVICE_ID_JOYPAD_L2,
                gilrs::Button::RightTrigger => DEVICE_ID_JOYPAD_R,
                gilrs::Button::RightTrigger2 => DEVICE_ID_JOYPAD_R2,
                gilrs::Button::Select => DEVICE_ID_JOYPAD_SELECT,
                gilrs::Button::Start => DEVICE_ID_JOYPAD_START,
                gilrs::Button::Mode => 0,
                gilrs::Button::LeftThumb => DEVICE_ID_JOYPAD_L3,
                gilrs::Button::RightThumb => DEVICE_ID_JOYPAD_R3,
                gilrs::Button::DPadUp => DEVICE_ID_JOYPAD_UP,
                gilrs::Button::DPadDown => DEVICE_ID_JOYPAD_DOWN,
                gilrs::Button::DPadLeft => DEVICE_ID_JOYPAD_LEFT,
                gilrs::Button::DPadRight => DEVICE_ID_JOYPAD_RIGHT,
                gilrs::Button::Unknown => 0,
            };

            if release {
                self.input_state &= !(1 << button);
            } else {
                self.input_state |= 1 << button;
            }
        }
    }

    pub(crate) fn input_state(
        &self,
        _port: c_uint,
        _device: c_uint,
        _index: c_uint,
        _id: c_uint,
    ) -> i16 {
        self.input_state
    }
}

#[derive(Debug)]
struct Variable {
    label: String,
    options: Vec<String>,
    default: String,
    value: CString,
}

impl Variable {
    fn parse(s: &str) -> Result<Self> {
        // TODO: potentially intern strings to make get_variable memory accesses more sound
        let (label, options) = s.split_once("; ").context("variable is missing `; `")?;
        let label = label.to_owned();
        let options = options.split('|').map(<_>::to_owned).collect::<Vec<_>>();
        let default = options.first().cloned().unwrap_or_default();
        let value = CString::new(default.clone()).context("BUG: value string contains NULL")?;

        Ok(Self {
            label,
            options,
            default,
            value,
        })
    }
}

pub unsafe extern "C" fn environment_cb(command: u32, data: *mut c_void) -> bool {
    let mut env = match ENVIRONMENT.lock() {
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

            env.set_pixel_format(pixel_format)
        }
        Command::GET_CAN_DUPE => {
            if !data.is_null() {
                *data.cast::<bool>() = true;
            }

            true
        }
        Command::SET_VARIABLES => {
            let mut variables = data.cast_const().cast::<libretro_sys::Variable>();
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
        Command::GET_VARIABLE => {
            let Some(variable) = data.cast::<libretro_sys::Variable>().as_mut() else {
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

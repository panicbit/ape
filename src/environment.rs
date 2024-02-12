use std::borrow::Cow;
use std::ffi::CString;
use std::sync::mpsc::{self, Receiver, SyncSender};

use anyhow::{anyhow, Context, Result};
use indexmap::IndexMap;
use libretro_sys::PixelFormat;

use crate::Frame;

pub struct Environment {
    variables: IndexMap<String, Variable>,
    pixel_format: PixelFormat,
    frame_sender: SyncSender<Frame>,
}

impl Environment {
    pub fn new() -> (Self, Receiver<Frame>) {
        let (tx, rx) = mpsc::sync_channel(1);
        let this = Self {
            pixel_format: PixelFormat::ARGB1555,
            variables: IndexMap::new(),
            frame_sender: tx,
        };

        (this, rx)
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
            .try_lock()
            .map_err(|err| anyhow!("BUG: failed to lock env while registering new env: {err}"))?;

        *env = Some(self);

        Ok(())
    }

    pub(crate) fn send_frame(&self, frame: Frame) {
        if self.frame_sender.try_send(frame).is_err() {
            eprintln!("Dropping frame, failed to send");
        }
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

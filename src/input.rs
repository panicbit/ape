use std::ffi::c_uint;

use enumset::EnumSetType;

pub mod gilrs;

#[derive(EnumSetType)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    Start,
    Select,
    L,
    L2,
    L3,
    R,
    R2,
    R3,
}

impl Button {
    pub fn from_raw_retro_joypad_device_id(device_id_joypad: c_uint) -> Option<Self> {
        Some(match device_id_joypad {
            libretro_sys::DEVICE_ID_JOYPAD_UP => Button::Up,
            libretro_sys::DEVICE_ID_JOYPAD_DOWN => Button::Down,
            libretro_sys::DEVICE_ID_JOYPAD_LEFT => Button::Left,
            libretro_sys::DEVICE_ID_JOYPAD_RIGHT => Button::Right,
            libretro_sys::DEVICE_ID_JOYPAD_A => Button::A,
            libretro_sys::DEVICE_ID_JOYPAD_B => Button::B,
            libretro_sys::DEVICE_ID_JOYPAD_X => Button::X,
            libretro_sys::DEVICE_ID_JOYPAD_Y => Button::Y,
            libretro_sys::DEVICE_ID_JOYPAD_SELECT => Button::Select,
            libretro_sys::DEVICE_ID_JOYPAD_START => Button::Start,
            libretro_sys::DEVICE_ID_JOYPAD_L => Button::L,
            libretro_sys::DEVICE_ID_JOYPAD_L2 => Button::L2,
            libretro_sys::DEVICE_ID_JOYPAD_L3 => Button::L3,
            libretro_sys::DEVICE_ID_JOYPAD_R => Button::R,
            libretro_sys::DEVICE_ID_JOYPAD_R2 => Button::R2,
            libretro_sys::DEVICE_ID_JOYPAD_R3 => Button::R3,
            _ => return None,
        })
    }
}

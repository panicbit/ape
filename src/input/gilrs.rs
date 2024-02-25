// use fnv::FnvHashMap;

impl super::Button {
    pub fn from_gilrs(button: gilrs::Button) -> Option<Self> {
        Some(match button {
            gilrs::Button::DPadUp => super::Button::Up,
            gilrs::Button::DPadDown => super::Button::Down,
            gilrs::Button::DPadLeft => super::Button::Left,
            gilrs::Button::DPadRight => super::Button::Right,
            gilrs::Button::East => super::Button::A,
            gilrs::Button::South => super::Button::B,
            gilrs::Button::North => super::Button::X,
            gilrs::Button::West => super::Button::Y,
            gilrs::Button::Start => super::Button::Start,
            gilrs::Button::Select => super::Button::Select,
            gilrs::Button::LeftTrigger => super::Button::L,
            gilrs::Button::LeftTrigger2 => super::Button::L2,
            gilrs::Button::LeftThumb => super::Button::L3,
            gilrs::Button::RightTrigger => super::Button::R,
            gilrs::Button::RightTrigger2 => super::Button::R2,
            gilrs::Button::RightThumb => super::Button::R3,
            _ => return None,
        })
    }
}

// pub fn default_button_mapping() -> FnvHashMap<gilrs::Button, super::Button> {
//     [
//         (gilrs::Button::DPadUp, super::Button::Up),
//         (gilrs::Button::DPadDown, super::Button::Down),
//         (gilrs::Button::DPadLeft, super::Button::Left),
//         (gilrs::Button::DPadRight, super::Button::Right),
//         (gilrs::Button::East, super::Button::A),
//         (gilrs::Button::South, super::Button::B),
//         (gilrs::Button::North, super::Button::X),
//         (gilrs::Button::West, super::Button::Y),
//         (gilrs::Button::Start, super::Button::Start),
//         (gilrs::Button::Select, super::Button::Select),
//         (gilrs::Button::LeftTrigger, super::Button::L),
//         (gilrs::Button::LeftTrigger2, super::Button::L2),
//         (gilrs::Button::LeftThumb, super::Button::L3),
//         (gilrs::Button::RightTrigger, super::Button::R),
//         (gilrs::Button::RightTrigger2, super::Button::R2),
//         (gilrs::Button::RightThumb, super::Button::R3),
//     ]
//     .into_iter()
//     .collect()
// }

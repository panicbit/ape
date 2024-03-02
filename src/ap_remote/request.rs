#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Request {
    #[serde(skip)]
    Version,
    Ping,
    System,
    PreferredCores,
    Hash,
    Guard {
        address: usize,
        #[serde(deserialize_with = "super::deserialize_base64")]
        expected_data: Vec<u8>,
        domain: String,
    },
    Lock,
    Unlock,
    Read {
        address: usize,
        size: usize,
        domain: String,
    },
    Write {
        address: usize,
        #[serde(deserialize_with = "super::deserialize_base64")]
        value: Vec<u8>,
    },
    DisplayMessage {
        message: String,
    },
    SetMessageInterval {
        value: u64,
    },
}

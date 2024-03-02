use std::collections::BTreeMap;

#[derive(serde::Serialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)]
pub enum Response {
    #[serde(skip)]
    Version,
    Pong,
    SystemResponse {
        value: String,
    },
    PreferredCoresResponse {
        value: BTreeMap<String, String>,
    },
    HashResponse {
        value: String,
    },
    GuardResponse {
        value: bool,
        address: usize,
    },
    Locked,
    Unlocked,
    ReadResponse {
        #[serde(serialize_with = "super::serialize_base64")]
        value: Vec<u8>,
    },
    WriteResponse,
    DisplayMessageResponse,
    SetMessageIntervalResponse,
    Error {
        err: String,
    },
}

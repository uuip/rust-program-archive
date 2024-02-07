use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct Msg {
    pub start: u64,
    pub stop: u64,
}

use dotenvy::dotenv;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Setting {
    pub pulsar_addr: String,
    pub topic: String,
    pub explorer_db: String,
    pub db: String,
    pub rpc_list: Vec<String>,
}

impl Setting {
    pub fn init() -> Self {
        dotenv().ok();
        envy::from_env::<Setting>().unwrap()
    }
}

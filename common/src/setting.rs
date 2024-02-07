#[derive(Debug)]
pub struct Setting {
    pub pulsar_addr: String,
    pub topic: String,
    pub explorer_db: String,
    pub db: String,
    pub rpc_list: Vec<String>,
}

pub fn get_env(key: &str) -> String {
    dotenvy::var(key).unwrap_or_else(|_| panic!("lost {key}"))
}

impl Setting {
    pub fn init() -> Self {
        let pulsar_addr = get_env("PULSAR_URL");
        let topic = get_env("PULSAR_TOPIC");
        let explorer_db = get_env("EXPLORER_DB");
        let db = get_env("DB_URL");
        let rpc_list = get_env("rpc_list")
            .split(',')
            .map(str::to_string)
            .collect::<Vec<String>>();
        Self {
            pulsar_addr,
            topic,
            explorer_db,
            db,
            rpc_list,
        }
    }
}

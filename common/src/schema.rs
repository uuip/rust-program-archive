use log::debug;
use pulsar::{
    producer, proto, DeserializeMessage, Error as PulsarError, Payload, SerializeMessage,
};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenMessageArg {
    pub from_user_id: String,
    pub to_user_id: String,
    pub coin_code: String,
    pub point: f32,
    pub tag_id: String,
    pub store_id: String,
    pub gen_time: String,
    #[serde(default = "default_pay_type")]
    pub pay_type: String,
    pub trxn_result: String,
    pub trxn_type: Option<String>,
}

fn default_pay_type() -> String {
    "xxPay".to_string()
}

impl TokenMessageArg {
    pub fn make_msg_with_ext(&self, ext_json: String) -> Msg {
        Msg {
            from_user_id: self.from_user_id.clone(),
            to_user_id: self.to_user_id.clone(),
            coin_code: self.coin_code.clone(),
            point: self.point,
            tag_id: self.tag_id.clone(),
            store_id: self.store_id.clone(),
            gen_time: self.gen_time.clone(),
            ext_json,
            retry_info: None,
        }
    }
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone)]
pub struct Msg {
    pub from_user_id: String,
    pub to_user_id: String,
    pub coin_code: String,
    pub point: f32,
    pub tag_id: String,
    pub store_id: String,
    pub gen_time: String,
    pub ext_json: String,
    pub retry_info: Option<String>,
}

impl SerializeMessage for Msg {
    fn serialize_message(input: Self) -> Result<producer::Message, PulsarError> {
        let payload = serde_json::to_vec(&input).map_err(|e| PulsarError::Custom(e.to_string()))?;
        Ok(producer::Message {
            payload,
            ..Default::default()
        })
    }
}

impl DeserializeMessage for Msg {
    type Output = Result<Msg, serde_json::Error>;

    fn deserialize_message(payload: &Payload) -> Self::Output {
        serde_json::from_slice(&payload.data)
    }
}

pub trait PulsarSchema
where
    Self: SerializeMessage,
    Self: DeserializeMessage,
{
    fn pulsar_json_schema() -> proto::Schema {
        let schema = schema_for!(Msg);
        let schema_map = serde_json::to_value(schema).unwrap();
        let fields: Vec<_> = schema_map["properties"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let t = if let Some(vf) = v.get("format") {
                    vf
                } else {
                    v.get("type").unwrap()
                };
                json!({"name":k,"type": t})
            })
            .collect();
        let myname = std::any::type_name::<Self>().split("::").last().unwrap();
        let pulsar_json_schema = json!(
                {"type": "record", "$id": "https://example.com/test.schema.json",
              "name": myname,"fields":fields});
        debug!("pulsar_json_schema {}", pulsar_json_schema);
        let schema_data = serde_json::to_vec(&pulsar_json_schema).unwrap();
        proto::Schema {
            schema_data,
            r#type: proto::schema::Type::Json as i32,
            ..Default::default()
        }
    }
}

impl<T> PulsarSchema for T where T: SerializeMessage + DeserializeMessage {}

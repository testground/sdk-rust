use serde::{Deserialize, Deserializer};
use serde_with::rust::string_empty_as_none;

fn json_empty_string_is_none<'de, D>(data: D) -> Result<Option<serde_json::Value>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Deserialize::deserialize(data)?;

    match value {
        Some(v) => {
            if v.is_string() && v.as_str().unwrap().is_empty() {
                Ok(None)
            } else {
                Ok(Some(v))
            }
        }
        None => Ok(None),
    }
}

#[derive(Deserialize, Debug)]
pub struct SignalEntry {
    pub seq: u64,
}

#[derive(Deserialize, Debug)]
pub struct Publish {
    pub seq: u64,
}

#[derive(Deserialize, Debug)]
pub struct RawResponse {
    pub id: String,

    #[serde(with = "string_empty_as_none")]
    pub error: Option<String>,

    #[serde(deserialize_with = "json_empty_string_is_none")]
    pub subscribe: Option<serde_json::Value>,

    pub signal_entry: Option<SignalEntry>,

    pub publish: Option<Publish>,
}

#[derive(Debug, PartialEq)]
pub enum ResponseType {
    SignalEntry { seq: u64 },
    Publish { seq: u64 },
    Subscribe(serde_json::Value),
    Error(String),
    Barrier,
}

#[derive(Debug, PartialEq)]
pub struct Response {
    pub id: String,
    pub response: ResponseType,
}

impl From<RawResponse> for Response {
    fn from(raw_response: RawResponse) -> Self {
        let RawResponse {
            id,
            error,
            subscribe,
            signal_entry,
            publish,
        } = raw_response;

        let response = match (error, subscribe, signal_entry, publish) {
            (None, None, None, None) => ResponseType::Barrier,
            (Some(error), None, None, None) => {
                //Hack to remove extra escape characters
                let error = serde_json::from_str(&error).expect("JSON Deserialization");
                ResponseType::Error(error)
            }
            (None, Some(msg), None, None) => ResponseType::Subscribe(msg),
            (None, None, Some(signal), None) => ResponseType::SignalEntry { seq: signal.seq },
            (None, None, None, Some(publish)) => ResponseType::Publish { seq: publish.seq },
            (error, subscribe, signal_entry, publish) => {
                panic!(
                    "Incompatible Raw Response {:?}",
                    RawResponse {
                        id,
                        error,
                        subscribe,
                        signal_entry,
                        publish,
                    }
                );
            }
        };

        Self { id, response }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_test() {
        let raw_response =
            "{\"id\":\"0\",\"error\":\"\",\"subscribe\":\"\",\"publish\":{\"seq\":1},\"signal_entry\":null}";

        let response: RawResponse = serde_json::from_str(raw_response).unwrap();

        let response: Response = response.into();

        assert_eq!(
            Response {
                id: "0".to_owned(),
                response: ResponseType::Publish { seq: 1 }
            },
            response
        );
    }
    #[test]
    fn serde_test_complex_subscribe() {
        let raw_response = "{\"id\":\"1\",\"error\":\"\",\"subscribe\":{\"Addrs\":[\"/ip4/16.3.0.3/tcp/45369\"],\"ID\":\"QmbSLMEMackm7vHiUGMB2EFAPbzeJNpeB9yTpzYKoojDWc\"}}";

        let response: RawResponse = serde_json::from_str(raw_response).unwrap();

        let response: Response = response.into();

        assert_eq!(
            Response {
                id: "1".to_owned(),
                response: ResponseType::Subscribe(serde_json::json!({
                    "Addrs": ["/ip4/16.3.0.3/tcp/45369"],
                    "ID": "QmbSLMEMackm7vHiUGMB2EFAPbzeJNpeB9yTpzYKoojDWc"
                }))
            },
            response
        );
    }
}

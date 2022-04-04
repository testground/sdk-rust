use serde::Deserialize;
use serde_with::rust::string_empty_as_none;

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

    #[serde(with = "string_empty_as_none")]
    pub subscribe: Option<String>,

    pub signal_entry: Option<SignalEntry>,

    pub publish: Option<Publish>,
}

#[derive(Debug, PartialEq)]
pub enum ResponseType {
    SignalEntry { seq: u64 },
    Publish { seq: u64 },
    Subscribe(String),
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
            (None, Some(msg), None, None) => {
                //Hack to remove extra escape characters
                let msg = serde_json::from_str(&msg).expect("JSON Deserialization");
                ResponseType::Subscribe(msg)
            }
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
        let raw_response = "{\"id\":\"0\",\"error\":\"\",\"subscribe\":\"\",\"publish\":{\"seq\":1},\"signal_entry\":null}";

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
}

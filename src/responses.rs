use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Response {
    pub id: String,

    pub error: Option<String>,

    #[serde(flatten)]
    pub response: Option<ResponseType>,
}

#[derive(Deserialize, Debug)]
pub enum ResponseType {
    #[serde(rename = "signal_entry")]
    SignalEntry { seq: u64 },
    #[serde(rename = "publish")]
    Publish { seq: u64 },
    #[serde(rename = "subscribe")]
    Subscribe(String),
}

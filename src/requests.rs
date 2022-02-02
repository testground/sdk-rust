use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct Request {
    pub id: String,

    pub is_cancel: bool,

    #[serde(flatten)]
    pub request: RequestType,
}

#[derive(Serialize, Debug)]
pub enum RequestType {
    #[serde(rename = "signal_entry")]
    SignalEntry { state: String },
    #[serde(rename = "barrier")]
    Barrier { state: String, target: u64 },
    #[serde(rename = "publish")]
    Publish { topic: String, payload: String },
    #[serde(rename = "subscribe")]
    Subscribe { topic: String },
}

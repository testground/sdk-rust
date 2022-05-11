#![allow(dead_code)]

use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct Event {
    pub event: EventType,
}

#[derive(Debug, Serialize)]
pub struct LogLine<'a> {
    pub ts: u128,
    pub event: &'a EventType,
}

#[derive(Serialize, Debug)]
pub enum EventType {
    #[serde(rename = "start_event")]
    Start { runenv: String },
    #[serde(rename = "message_event")]
    Message { message: String },
    #[serde(rename = "success_event")]
    Success { group: String },
    #[serde(rename = "failure_event")]
    Failure { group: String, error: String },
    #[serde(rename = "crash_event")]
    Crash {
        groups: String,
        error: String,
        stacktrace: String,
    },
    #[serde(rename = "stage_start_event")]
    StageStart { name: String, group: String },
    #[serde(rename = "stage_end_event")]
    StageEnd { name: String, group: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_test() {
        //let raw_response = r#"{"key": "run:c7uji38e5te2b9t464v0:plan:streaming_test:case:quickstart:run_events", "error": "failed to decode as type *runtime.Event: \"{\\\"stage_end_event\\\":{\\\"name\\\":\\\"network-initialized\\\",\\\"group\\\":\\\"single\\\"}}\"", "id": "0"}"#;

        let event = Event {
            event: EventType::StageStart {
                name: "network-initialized".to_owned(),
                group: "single".to_owned(),
            },
        };

        let json = serde_json::to_string(&event).unwrap();

        println!("{:?}", json);
    }
}

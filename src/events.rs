#![allow(dead_code)]

use serde::Serialize;

#[derive(Serialize, Debug)]
pub enum Event {
    #[serde(rename = "start_event")]
    Start { runenv: String },
    #[serde(rename = "message_event")]
    Message { message: String },
    #[serde(rename = "success_event")]
    Success { groups: String },
    #[serde(rename = "failure_event")]
    Failure { groups: String, error: String },
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

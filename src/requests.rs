use serde::Serialize;

use crate::{events::Event, network_conf::NetworkConfiguration};

#[derive(Serialize, Debug)]
pub struct Request {
    pub id: String,

    pub is_cancel: bool,

    #[serde(flatten)]
    pub request: RequestType,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum PlayloadType {
    Event(Event),

    String(String),

    Config(NetworkConfiguration),
}

#[derive(Serialize, Debug)]
pub enum RequestType {
    #[serde(rename = "signal_entry")]
    SignalEntry { state: String },
    #[serde(rename = "barrier")]
    Barrier { state: String, target: u64 },
    #[serde(rename = "publish")]
    Publish {
        topic: String,

        payload: PlayloadType,
    },
    #[serde(rename = "subscribe")]
    Subscribe { topic: String },
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use durationfmt::to_string;

    use crate::network_conf::*;

    use super::*;

    #[test]
    fn serde_test() {
        let network_conf = NetworkConfiguration {
            network: DEAFULT_DATA_NETWORK.to_owned(),
            ipv4: None,
            enable: true,
            default: LinkShape {
                latency: to_string(Duration::from_millis(50)),
                jitter: to_string(Duration::from_millis(5)),
                bandwidth: 2000,
                filter: FilterAction::Accept,
                loss: 1.0,
                corrupt: 1.0,
                corrupt_corr: 1.0,
                reorder: 1.0,
                reorder_corr: 1.0,
                duplicate: 1.0,
                duplicate_corr: 1.0,
            },
            rules: vec![],
            callback_state: "trafic".to_owned(),
            callback_target: 10,
            routing_policy: RoutingPolicyType::AllowAll,
        };

        /* let event = Event::StageStart {
            name: "network-initialized".to_owned(),
            group: "single".to_owned(),
        }; */

        //let msg = "123QM 192.168.1.1/25".to_owned();

        let req = Request {
            id: "0".to_owned(),
            is_cancel: false,
            request: RequestType::Publish {
                topic: "run:abcd1234:plan:live_streming:case:quickstart:topics:network:hostname"
                    .to_owned(),
                payload: PlayloadType::Config(network_conf),
            },
        };

        let json_req = serde_json::to_string_pretty(&req).unwrap();

        println!("{}", json_req);
    }
}

use serde::Serialize;

use crate::{events::EventType, network_conf::NetworkConfiguration};

#[derive(Serialize, Debug)]
pub struct Request {
    pub id: String,

    pub is_cancel: bool,

    #[serde(flatten)]
    pub request: RequestType,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum PayloadType {
    Event(EventType),

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
    Publish { topic: String, payload: PayloadType },
    #[serde(rename = "subscribe")]
    Subscribe { topic: String },
}

#[cfg(test)]
mod tests {

    use std::net::Ipv4Addr;

    use ipnetwork::Ipv4Network;

    use crate::{events::EventType, network_conf::*};

    use super::*;

    #[test]
    fn serde_test() {
        let _network_conf = NetworkConfiguration {
            network: DEAFULT_DATA_NETWORK.to_owned(),
            ipv4: Some(Ipv4Network::new(Ipv4Addr::new(16, 0, 1, 1), 24).unwrap()),
            ipv6: None,
            enable: true,
            default: LinkShape {
                latency: 10000000,
                jitter: 0,
                bandwidth: 1048576,
                filter: FilterAction::Accept,
                loss: 0.0,
                corrupt: 0.0,
                corrupt_corr: 0.0,
                reorder: 0.0,
                reorder_corr: 0.0,
                duplicate: 0.0,
                duplicate_corr: 0.0,
            },
            rules: None,
            callback_state: "latency-reduced".to_owned(),
            callback_target: None,
            routing_policy: RoutingPolicyType::DenyAll,
        };

        let event = EventType::StageStart {
            name: "network-initialized".to_owned(),
            group: "single".to_owned(),
        };

        let _msg = "123QM 192.168.1.1/25".to_owned();

        let req = Request {
            id: "0".to_owned(),
            is_cancel: false,
            request: RequestType::Publish {
                topic: "run:abcd1234:plan:live_streming:case:quickstart:topics:network:hostname"
                    .to_owned(),
                payload: PayloadType::Event(event),
            },
        };

        let json_req = serde_json::to_string_pretty(&req).unwrap();

        println!("{}", json_req);
    }
}

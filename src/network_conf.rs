#![allow(dead_code)]

use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use serde::Serialize;

use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug)]
#[repr(u8)]
pub enum FilterAction {
    Accept = 0,
    Reject = 1,
    Drop = 2,
}

#[derive(Serialize, Debug)]
/// LinkShape defines how traffic should be shaped.
pub struct LinkShape {
    /// Latency is the egress latency.
    ///
    /// Egg; 10s, 50ms, etc...
    pub latency: String,

    /// Jitter is the egress jitter.
    ///
    /// Egg; 10s, 50ms, etc...
    pub jitter: String,

    /// Bandwidth is egress bytes per second.
    pub bandwidth: u64,

    /// Drop all inbound traffic.
    /// TODO: Not implemented
    pub filter: FilterAction,

    /// Loss is the egress packet loss (%)
    pub loss: f32,

    /// Corrupt is the egress packet corruption probability (%)
    pub corrupt: f32,

    /// Corrupt is the egress packet corruption correlation (%)
    pub corrupt_corr: f32,

    /// Reorder is the probability that an egress packet will be reordered (%)
    ///
    /// Reordered packets will skip the latency delay and be sent
    /// immediately. You must specify a non-zero Latency for this option to
    /// make sense.
    pub reorder: f32,

    /// ReorderCorr is the egress packet reordering correlation (%)
    pub reorder_corr: f32,

    /// Duplicate is the percentage of packets that are duplicated (%)
    pub duplicate: f32,

    /// DuplicateCorr is the correlation between egress packet duplication (%)
    pub duplicate_corr: f32,
}

#[derive(Serialize, Debug)]
/// LinkRule applies a LinkShape to a subnet.
pub struct LinkRule {
    #[serde(flatten)]
    pub link_shape: LinkShape,

    pub subnet: IpNetwork,
}

pub const DEAFULT_DATA_NETWORK: &str = "default";

#[derive(Serialize, Debug)]
pub enum RoutingPolicyType {
    #[serde(rename = "allow_all")]
    AllowAll,
    #[serde(rename = "deny_all")]
    DenyAll,
}

/// NetworkConfiguration specifies how a node's network should be configured.
#[derive(Serialize, Debug)]
pub struct NetworkConfiguration {
    /// Network is the name of the network to configure.
    pub network: String,

    /// IPv4 and IPv6 set the IP addresses of this network device. If
    /// unspecified, the sidecar will leave them alone.
    ///
    /// Your test-case will be assigned a B block in the range
    /// 16.0.0.1-32.0.0.0. X.Y.0.1 will always be reserved for the gateway
    /// and shouldn't be used by the test.
    ///
    /// TODO: IPv6 is currently not supported.
    pub ipv4: Option<Ipv4Network>,

    /// Enable enables this network device.
    pub enable: bool,

    /// Default is the default link shaping rule.
    pub default: LinkShape,

    /// Rules defines how traffic should be shaped to different subnets.
    ///
    /// TODO: This is not implemented.
    pub rules: Vec<LinkRule>,

    /// CallbackState will be signalled when the link changes are applied.
    ///
    /// Nodes can use the same state to wait for _all_ or a subset of nodes to
    /// enter the desired network state. See CallbackTarget.
    pub callback_state: String,

    /// CallbackTarget is the amount of instances that will have needed to signal
    /// on the Callback state to consider the configuration operation a success.
    #[serde(rename = "-")]
    pub callback_target: u64,

    /// RoutingPolicy defines the data routing policy of a certain node. This affects
    /// external networks other than the network 'Default', e.g., external Internet
    /// access.
    pub routing_policy: RoutingPolicyType,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use durationfmt::to_string;

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

        let json = serde_json::to_string_pretty(&network_conf).unwrap();

        println!("{}", json);
    }
}

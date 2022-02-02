#![allow(dead_code)]

use std::{net::Ipv4Addr, time::Duration};

use pnet::ipnetwork::IpNetwork;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub enum FilterAction {
    Accept,
    Reject,
    Drop,
}

#[derive(Serialize, Debug)]
/// LinkShape defines how traffic should be shaped.
pub struct LinkShape {
    /// Latency is the egress latency
    pub latency: Duration,

    /// Jitter is the egress jitter
    pub jitter: Duration,

    /// Bandwidth is egress bytes per second
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
    pub link_shape: LinkShape,

    #[serde(rename = "Subnet")]
    pub sub_net: IpNetwork,
}

/// NetworkConfiguration specifies how a node's network should be configured.
#[derive(Serialize, Debug)]
pub struct NetworkConfiguration {
    /// Network is the name of the network to configure
    #[serde(rename = "Network")]
    pub network: String,

    /// IPv4 and IPv6 set the IP addresses of this network device. If
    /// unspecified, the sidecar will leave them alone.
    ///
    /// Your test-case will be assigned a B block in the range
    /// 16.0.0.1-32.0.0.0. X.Y.0.1 will always be reserved for the gateway
    /// and shouldn't be used by the test.
    ///
    /// TODO: IPv6 is currently not supported.
    #[serde(rename = "IPv4")]
    pub ip_v4: Ipv4Addr,

    /// Enable enables this network device.
    #[serde(rename = "Enable")]
    pub enable: bool,

    /// Default is the default link shaping rule.
    #[serde(rename = "Default")]
    pub default: LinkShape,

    /// Rules defines how traffic should be shaped to different subnets.
    ///
    /// TODO: This is not implemented.
    #[serde(rename = "Rules")]
    pub rules: Vec<LinkRule>,

    /// CallbackState will be signalled when the link changes are applied.
    ///
    /// Nodes can use the same state to wait for _all_ or a subset of nodes to
    /// enter the desired network state. See CallbackTarget.
    #[serde(rename = "State")]
    pub callback_state: String,

    /// CallbackTarget is the amount of instances that will have needed to signal
    /// on the Callback state to consider the configuration operation a success.
    #[serde(rename = "-")]
    pub callback_target: u64,
}

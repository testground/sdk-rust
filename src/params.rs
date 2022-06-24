use clap::Parser;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

use std::path::PathBuf;

use ipnetwork::IpNetwork;

#[derive(Parser, Debug, Clone)]
/// RunParameters encapsulates the runtime parameters for this test.
pub struct RunParameters {
    #[clap(env)]
    pub test_plan: String, // TEST_PLAN: streaming_test
    #[clap(env)]
    pub test_case: String, // TEST_CASE: quickstart
    #[clap(env)]
    pub test_run: String, // TEST_RUN: c7fjstge5te621cen4i0

    #[clap(env)]
    pub test_repo: String, //TEST_REPO:
    #[clap(env)]
    pub test_branch: String, // TEST_BRANCH:
    #[clap(env)]
    pub test_tag: String, // TEST_TAG:

    #[clap(env)]
    pub test_outputs_path: PathBuf, // TEST_OUTPUTS_PATH: /outputs
    #[clap(env)]
    pub test_temp_path: String, // TEST_TEMP_PATH: /temp

    #[clap(env)]
    pub test_instance_count: u64, // TEST_INSTANCE_COUNT: 1
    #[clap(env)]
    pub test_instance_role: String, // TEST_INSTANCE_ROLE:
    #[clap(env, parse(try_from_str = parse_key_val))]
    pub test_instance_params: HashMap<String, String>, // TEST_INSTANCE_PARAMS: feature=false|neutral_nodes=10|num=2|word=never

    #[clap(long, env)]
    pub test_sidecar: bool, // TEST_SIDECAR: true

    #[clap(env)]
    pub test_subnet: IpNetwork, // TEST_SUBNET: 16.0.0.0/16
    #[clap(env)]
    pub test_start_time: String, // TEST_START_TIME: 2022-01-12T15:48:07-05:00

    #[clap(env)]
    pub test_capture_profiles: String, // TEST_CAPTURE_PROFILES:

    #[clap(env)]
    pub test_group_instance_count: u64, // TEST_GROUP_INSTANCE_COUNT: 1
    #[clap(env)]
    pub test_group_id: String, // TEST_GROUP_ID: single

    #[clap(long, env)]
    pub test_disable_metrics: bool, // TEST_DISABLE_METRICS: false

    #[clap(env)]
    pub hostname: String, // HOSTNAME: e6f4cc8fc147
    #[clap(env)]
    pub influxdb_url: String, // INFLUXDB_URL: http://testground-influxdb:8086
    #[clap(env)]
    pub redis_host: String, // REDIS_HOST: testground-redis
                            // HOME: /
}

impl RunParameters {
    /// Examines the local network interfaces, and tries to find our assigned IP
    /// within the data network.
    ///
    /// If running in a sidecar-less environment, the loopback address is
    /// returned.
    pub fn data_network_ip(&self) -> std::io::Result<Option<IpAddr>> {
        if !self.test_sidecar {
            // This must be a local:exec runner and we currently don't support
            // traffic shaping on it for now, just return the loopback address.
            return Ok(Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        }

        Ok(if_addrs::get_if_addrs()?
            .into_iter()
            .map(|i| i.addr.ip())
            .find(|ip| self.test_subnet.contains(*ip)))
    }
}

fn parse_key_val(s: &str) -> Result<HashMap<String, String>, String> {
    let mut hashmap = HashMap::new();

    for kv in s.split('|').filter(|&s| !s.is_empty()) {
        let pos = kv
            .find('=')
            .ok_or_else(|| format!("Invalid KEY=VALUE: no '=' found in {}", kv))?;
        hashmap.insert(String::from(&kv[..pos]), String::from(&kv[pos + 1..]));
    }

    Ok(hashmap)
}

#[test]
fn test_parse_key_val() {
    let result = parse_key_val("feature=false|neutral_nodes=10|num=2|word=never").unwrap();
    assert_eq!(4, result.len());
    assert_eq!("false", result.get("feature").unwrap());
    assert_eq!("10", result.get("neutral_nodes").unwrap());
    assert_eq!("2", result.get("num").unwrap());
    assert_eq!("never", result.get("word").unwrap());

    let result = parse_key_val("feature=false").unwrap();
    assert_eq!(1, result.len());
    assert_eq!("false", result.get("feature").unwrap());

    let result = parse_key_val("word=ne=ver").unwrap();
    assert_eq!(1, result.len());
    assert_eq!("ne=ver", result.get("word").unwrap());

    let result = parse_key_val("").unwrap();
    assert!(result.is_empty());

    let result = parse_key_val("feature=false|neutral_nodes");
    assert!(result.is_err());
}

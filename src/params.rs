use clap::Parser;

use std::path::PathBuf;

use ipnetwork::IpNetwork;

#[derive(Parser, Debug, Clone)]
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
    #[clap(env)]
    pub test_instance_params: String, // TEST_INSTANCE_PARAMS: feature=false|neutral_nodes=10|num=2|word=never

    #[clap(long, env)]
    pub test_sidecar: bool, // TEST_SIDECAR: true

    #[clap(env)]
    pub test_subnet: IpNetwork, // TEST_SUBNET: 16.0.0.0/16
    #[clap(env)]
    pub test_start_time: String, // TEST_START_TIME: 2022-01-12T15:48:07-05:00

    #[clap(env)]
    pub test_capture_profiles: String, // TEST_CAPTURE_PROFILES:

    #[clap(env)]
    pub test_group_instance_count: usize, // TEST_GROUP_INSTANCE_COUNT: 1
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

use std::path::PathBuf;

use pnet::ipnetwork::IpNetwork;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct RunParameters {
    #[structopt(env)]
    pub test_plan: String, // TEST_PLAN: streaming_test
    #[structopt(env)]
    pub test_case: String, // TEST_CASE: quickstart
    #[structopt(env)]
    pub test_run: String, // TEST_RUN: c7fjstge5te621cen4i0

    #[structopt(env)]
    pub test_repo: Option<String>, //TEST_REPO:
    #[structopt(env)]
    pub test_branch: Option<String>, // TEST_BRANCH:
    #[structopt(env)]
    pub test_tag: Option<String>, // TEST_TAG:

    #[structopt(env)]
    pub test_outputs_path: Option<PathBuf>, // TEST_OUTPUTS_PATH: /outputs
    #[structopt(env)]
    pub test_temp_path: Option<String>, // TEST_TEMP_PATH: /temp

    #[structopt(env)]
    pub test_instance_count: u64, // TEST_INSTANCE_COUNT: 1
    #[structopt(env)]
    pub test_instance_role: Option<String>, // TEST_INSTANCE_ROLE:
    #[structopt(env)]
    pub test_instance_params: Option<String>, // TEST_INSTANCE_PARAMS:

    #[structopt(long, env)]
    pub test_sidecar: bool, // TEST_SIDECAR: true

    #[structopt(env)]
    pub test_subnet: Option<IpNetwork>, // TEST_SUBNET: 16.0.0.0/16
    #[structopt(env)]
    pub test_start_time: Option<String>, // TEST_START_TIME: 2022-01-12T15:48:07-05:00

    #[structopt(env)]
    pub test_capture_profiles: Option<String>, // TEST_CAPTURE_PROFILES:

    #[structopt(env)]
    pub test_group_instance_count: Option<usize>, // TEST_GROUP_INSTANCE_COUNT: 1
    #[structopt(env)]
    pub test_group_id: String, // TEST_GROUP_ID: single

    #[structopt(env)]
    pub test_disable_metrics: Option<bool>, // TEST_DISABLE_METRICS: false

    #[structopt(env)]
    pub hostname: Option<String>, // HOSTNAME: e6f4cc8fc147
    #[structopt(env)]
    pub influxdb_url: Option<String>, // INFLUXDB_URL: http://testground-influxdb:8086
    #[structopt(env)]
    pub redis_host: Option<String>, // REDIS_HOST: testground-redis
                                    // HOME: /
}

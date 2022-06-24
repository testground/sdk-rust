#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = testground::client::Client::new_and_init().await?;

    client.record_message(format!(
        "{}, sdk-rust!",
        client
            .run_parameters
            .test_instance_params
            .get("greeting")
            .unwrap()
    ));

    client.record_success().await?;

    Ok(())
}

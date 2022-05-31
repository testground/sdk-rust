#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (client, run_parameters) = testground::client::Client::new().await?;

    client.record_message(format!(
        "{}, sdk-rust!",
        run_parameters.test_instance_params.get("greeting").unwrap()
    ));

    client.record_success().await?;

    Ok(())
}

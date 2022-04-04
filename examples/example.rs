#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _run_parameters) = testground::client::Client::new().await?;

    client.record_success().await?;

    Ok(())
}

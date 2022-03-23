#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sync_client = testground::sync::Client::new().await?;

    sync_client.publish_success().await?;

    Ok(())
}

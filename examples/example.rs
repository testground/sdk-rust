use std::borrow::Cow;

use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = testground::client::Client::new_and_init().await?;

    match client.run_parameters().test_case.as_str() {
        "example" => example(client).await,
        "publish-subscribe" => publish_subscribe(client).await,
        _ => panic!("Unknown test case: {}", client.run_parameters().test_case),
    }
}

async fn example(client: testground::client::Client) -> Result<(), Box<dyn std::error::Error>> {
    client.record_message(format!(
        "{}, sdk-rust!",
        client
            .run_parameters()
            .test_instance_params
            .get("greeting")
            .unwrap()
    ));

    client.record_success().await?;

    Ok(())
}

async fn publish_subscribe(
    client: testground::client::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    client.record_message("running the publish_subscribe test");

    match client.global_seq() {
        1 => {
            client.record_message("I am instance 1: acting as the leader");

            let json = serde_json::json!({"foo": "bar"});
            client.publish("demonstration", Cow::Owned(json)).await?;
            client.record_success().await?;
        }
        _ => {
            client.record_message(format!(
                "I am instance {}: acting as a follower",
                client.global_seq()
            ));

            let payload = client
                .subscribe("demonstration")
                .await
                .take(1)
                .map(|x| x.unwrap())
                .next()
                .await
                .unwrap();

            client.record_message(format!("I received the payload: {}", payload));

            if payload["foo"].as_str() == Some("bar") {
                client.record_success().await?;
            } else {
                client
                    .record_failure(format!("invalid payload: {}", payload))
                    .await?;
            }
        }
    }
    Ok(())
}

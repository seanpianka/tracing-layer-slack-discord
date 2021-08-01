use crate::types::ChannelReceiver;
use crate::WorkerMessage;

/// Provides a background worker task that sends the messages generated by the layer.
pub async fn worker(mut rx: ChannelReceiver) {
    let client = reqwest::Client::new();
    while let Some(message) = rx.recv().await {
        match message {
            WorkerMessage::Data(payload) => {
                let webhook_url = payload.webhook_url().clone();
                let payload = serde_json::to_string(&payload).expect("failed to deserialize slack payload, this is a bug");
                match client.post(webhook_url).body(payload).send().await {
                    Ok(res) => {
                        println!("{}", res.text().await.unwrap());
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                };
            }
            WorkerMessage::Shutdown => {
                break;
            }
        }
    }
}
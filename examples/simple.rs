use std::time::Duration;

use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{SlackConfig, SlackForwardingLayer, WorkerMessage};

#[instrument]
pub async fn create_user(id: u64) {
    for i in 0..2 {
        network_io(i).await;
    }
    info!(param = id, "user created");
}

#[instrument]
pub async fn network_io(id: u64) {
    info!(id, "did a network i/o thing");
}

pub async fn handler() {
    info!("Orphan event without a parent span");
    create_user(2).await;
    tokio::time::sleep(Duration::from_secs(10)).await;
    create_user(4).await;
    tokio::time::sleep(Duration::from_secs(10)).await;
    create_user(6).await;
}

#[tokio::main]
async fn main() {
    let (slack_layer, channel_sender, background_worker) = SlackForwardingLayer::new("simple".into(), SlackConfig::default());
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let handle = tokio::spawn(background_worker);
    handler().await;
    channel_sender.send(WorkerMessage::Shutdown).unwrap();
    handle.await.unwrap();
}
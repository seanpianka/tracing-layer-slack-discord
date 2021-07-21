use tokio::time::{Duration, sleep};
use tracing::info;
use tracing::instrument;
use tracing_bunyan_formatter::JsonStorageLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

use tracing_layer_slack::SlackForwardingLayer;

#[instrument]
pub async fn a_unit_of_work(_first_parameter: u64) {
    for i in 0..2 {
        a_sub_unit_of_work(i);
    }
    info!(excited = "true", "Tracing is quite cool!");
}

#[instrument]
pub fn a_sub_unit_of_work(_sub_parameter: u64) {
    info!("Events have the full context of their parent span!");
}

#[tokio::main]
async fn main() {
    let slack_layer = SlackForwardingLayer::new_from_env();
    let subscriber = Registry::default().with(JsonStorageLayer).with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    info!("Orphan event without a parent span");
    a_unit_of_work(2).await;
    sleep(Duration::from_secs(5)).await;
}

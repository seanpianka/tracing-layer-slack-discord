use regex::Regex;
use tracing::{info, instrument, warn};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, SlackLayer};

#[instrument]
pub async fn create_user(id: u64) {
    network_io(id).await;
    info!(param = id, "A user was created");
}

#[instrument]
pub async fn network_io(id: u64) {
    warn!(user_id = id, "had to retry the request once");
}

pub async fn controller() {
    info!("Orphan event without a parent span");
    tokio::join!(create_user(2), create_user(4), create_user(6));
}

#[tokio::main]
async fn main() {
    // Only show events from where this example code is the target.
    let target_to_filter: EventFilters = Regex::new("simple").unwrap().into();

    let (slack_layer, background_worker) = SlackLayer::builder(target_to_filter).build();
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    controller().await;
    background_worker.shutdown().await;
}

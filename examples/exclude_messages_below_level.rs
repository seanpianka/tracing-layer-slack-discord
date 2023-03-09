use regex::Regex;
use tracing::{debug, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, SlackLayer};

#[instrument]
pub async fn handler() {
    info!("this is the only message you should see");
    debug!("this should be excluded");
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = Regex::new("exclude_messages_below_level").unwrap().into();
    let (slack_layer, background_worker) = SlackLayer::builder(targets_to_filter)
        .level_filters("info".to_string())
        .build();
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    handler().await;
    background_worker.shutdown().await;
}

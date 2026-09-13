use regex::Regex;
use tracing::{debug, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, LevelFilter, SlackLayer};

#[instrument]
pub async fn handler() {
    info!("this is the only message you should see");
    debug!("this should be excluded");
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = Regex::new("exclude_messages_below_level").unwrap().into();
    let (slack_layer, delivery) = SlackLayer::from_env("test-app", targets_to_filter)
        .expect("valid Slack webhook configuration")
        .level_filter(LevelFilter::INFO)
        .build();
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let delivery = delivery.spawn().expect("active Tokio runtime");
    handler().await;
    delivery.shutdown().await.expect("Slack delivery shutdown");
}

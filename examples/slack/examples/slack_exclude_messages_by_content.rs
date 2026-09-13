use regex::Regex;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, SlackLayer};

#[instrument]
pub async fn handler() {
    info!("the message we want to exclude");
    info!("this should be shown");
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = (None, None).into();
    let messages_to_exclude = vec![Regex::new("the message we want to exclude").unwrap()];
    let (slack_layer, delivery) = SlackLayer::from_env("test-app", targets_to_filter)
        .expect("valid Slack webhook configuration")
        .message_filters((Vec::new(), messages_to_exclude).into())
        .build();
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let delivery = delivery.spawn().expect("active Tokio runtime");
    handler().await;
    delivery.shutdown().await.expect("Slack delivery shutdown");
}

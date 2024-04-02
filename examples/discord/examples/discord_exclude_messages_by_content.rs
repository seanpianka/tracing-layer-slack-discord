use regex::Regex;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_discord::{EventFilters, DiscordLayer};

#[instrument]
pub async fn handler() {
    info!("the message we want to exclude");
    info!("this should be shown");
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = (None, None).into();
    let messages_to_exclude = vec![Regex::new("the message we want to exclude").unwrap()];
    let (discord_layer, background_worker) = DiscordLayer::builder("test-app".to_string(), targets_to_filter)
        .message_filters((Vec::new(), messages_to_exclude).into())
        .build();
    let subscriber = Registry::default().with(discord_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    handler().await;
    background_worker.shutdown().await;
}

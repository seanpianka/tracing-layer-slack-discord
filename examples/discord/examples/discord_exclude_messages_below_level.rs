use regex::Regex;
use tracing::{debug, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_discord::{EventFilters, DiscordLayer};

#[instrument]
pub async fn handler() {
    info!("this is the only message you should see");
    debug!("this should be excluded");
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = Regex::new("exclude_messages_below_level").unwrap().into();
    let (discord_layer, background_worker) = DiscordLayer::builder("test-app".to_string(), targets_to_filter)
        .level_filters("info".to_string())
        .build();
    let subscriber = Registry::default().with(discord_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    handler().await;
    background_worker.shutdown().await;
}

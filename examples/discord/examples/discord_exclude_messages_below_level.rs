use regex::Regex;
use tracing::{debug, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_discord::{DiscordLayer, EventFilters, LevelFilter};

#[instrument]
pub async fn handler() {
    info!("this is the only message you should see");
    debug!("this should be excluded");
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = Regex::new("exclude_messages_below_level").unwrap().into();
    let (discord_layer, delivery) = DiscordLayer::from_env("test-app", targets_to_filter)
        .expect("valid Discord webhook configuration")
        .level_filter(LevelFilter::INFO)
        .build();
    let subscriber = Registry::default().with(discord_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let delivery = delivery.spawn().expect("active Tokio runtime");
    handler().await;
    delivery.shutdown().await.expect("Discord delivery shutdown");
}

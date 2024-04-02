use regex::Regex;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_discord::{EventFilters, DiscordLayer};

#[instrument]
pub async fn create_user(id: u64, password: String) {
    info!(param = id, %password, "user created");
}

pub async fn handler() {
    create_user(2, "hunter2".into()).await;
    info!("this is the only trace you should see");
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = Regex::new("filter_records_by_fields").unwrap().into();
    let event_fields_to_filter: EventFilters = Regex::new("password").unwrap().into();
    let (discord_layer, background_worker) = DiscordLayer::builder("test-app".to_string(), targets_to_filter)
        .event_by_field_filters(event_fields_to_filter)
        .build();
    let subscriber = Registry::default().with(discord_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    handler().await;
    background_worker.shutdown().await;
}

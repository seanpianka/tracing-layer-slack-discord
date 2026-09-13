use regex::Regex;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_discord::{DiscordLayer, EventFilters};

#[instrument]
pub async fn handler() {
    info!(
        the_only_field = "this is the only field you should see",
        password = "hunter2",
        access_token = "sifjsadfjasd89fuas8d9f",
        command = "Authenticate { \"username\": \"blah\", \"password\": \"asdf\""
    );
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = Regex::new("exclude_fields_from_messages").unwrap().into();
    let fields_to_exclude = vec![
        Regex::new(".*token.*").unwrap(),
        Regex::new(".*password.*").unwrap(),
        Regex::new("command").unwrap(),
    ];
    let (discord_layer, delivery) = DiscordLayer::from_env("test-app", targets_to_filter)
        .expect("valid Discord webhook configuration")
        .field_exclusion_filters(fields_to_exclude)
        .build();
    let subscriber = Registry::default().with(discord_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let delivery = delivery.spawn().expect("active Tokio runtime");
    handler().await;
    delivery.shutdown().await.expect("Discord delivery shutdown");
}

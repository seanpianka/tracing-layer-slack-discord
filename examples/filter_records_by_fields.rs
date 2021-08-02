use regex::Regex;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, SlackLayer};

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
    let (slack_layer, mut background_worker) = SlackLayer::builder(targets_to_filter)
        .event_by_field_filters(event_fields_to_filter)
        .build();
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    background_worker.startup().await;
    handler().await;
    background_worker.teardown().await;
}

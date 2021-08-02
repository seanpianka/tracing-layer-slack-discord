use regex::Regex;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, SlackLayer, WorkerMessage};

#[instrument]
pub async fn handler() {
    info!(
        the_only_field = "this is the only field you should see",
        password = "hunter2"
    );
}

#[tokio::main]
async fn main() {
    let targets_to_filter: EventFilters = Regex::new("exclude_fields_from_messages").unwrap().into();
    let fields_to_exclude = vec![Regex::new("password").unwrap()];
    let (slack_layer, background_worker, channel_sender) = SlackLayer::builder(targets_to_filter)
        .field_exclusion_filters(fields_to_exclude)
        .build();
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let handle = tokio::spawn(background_worker);
    handler().await;
    channel_sender.send(WorkerMessage::Shutdown).unwrap();
    handle.await.unwrap();
}

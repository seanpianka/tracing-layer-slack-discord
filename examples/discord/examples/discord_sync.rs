use tracing::{info, instrument, warn, warn_span};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_discord::DiscordLayer;

#[instrument]
pub async fn create_user(id: u64) {
    app_users_webhook(id).await;
    info!(param = id, "A user was created");
}

#[instrument(fields(electric_utilityaccount_id))]
pub async fn app_users_webhook(id: u64) {
    tracing::Span::current().record("electric_utilityaccount_id", id);
    warn!(
        met = r#"
        John Baker
        "#,
        r#"error parsing user event by webhook handler: failed to parse event metadata: none found"#
    );
}

#[instrument]
pub async fn controller() {
    info!("Orphan event without a parent span");
    app_users_webhook(2).await;
    // tokio::join!(create_user(2), create_user(4), create_user(6));
}

fn main() {
    let formatting_layer = tracing_bunyan_formatter::BunyanFormattingLayer::new("tracing_demo".into(), std::io::stdout);
    let (discord_layer, background_worker) = DiscordLayer::builder("test-app".to_string(), Default::default()).build();
    let subscriber = Registry::default()
        .with(discord_layer)
        .with(tracing_bunyan_formatter::JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async move {
        background_worker.start().await;
        controller().await;
        background_worker.shutdown().await;
    });
}

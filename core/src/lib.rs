use std::fmt::Debug;

use serde_json::Value;
use tracing::Level;

pub use filters::EventFilters;
pub use worker::BackgroundWorker;
pub use worker::WorkerMessage;

mod aws_lambda;
pub mod filters;
pub mod layer;
mod worker;

pub type ChannelSender = tokio::sync::mpsc::UnboundedSender<WorkerMessage>;
pub type ChannelReceiver = tokio::sync::mpsc::UnboundedReceiver<WorkerMessage>;

/// Send a message to a webhook endpoint.
pub trait WebhookMessage: Debug + Send + Sync {
    fn webhook_url(&self) -> &str;
    fn serialize(&self) -> String;
}

pub trait WebhookMessageFactory {
    fn create(inputs: WebhookMessageInputs) -> impl WebhookMessage;
}

/// The data expected to be available for message producers.
pub struct WebhookMessageInputs {
    pub app_name: String,
    pub message: String,
    pub target: String,
    pub span: String,
    pub metadata: String,
    pub webhook_url: String,
    pub source_line: u32,
    pub source_file: String,
    pub event_level: Level,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum WebhookMessageSpec {
    TextNoEmbed(String),
    TextWithEmbed(String, Vec<Value>),
    EmbedNoText(Vec<Value>),
}

pub trait Config {
    fn webhook_url(&self) -> &str;

    fn new_from_env() -> Self
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {}

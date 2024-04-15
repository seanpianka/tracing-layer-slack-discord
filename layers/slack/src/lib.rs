#![doc = include_str!("../README.md")]

pub use tracing_layer_core::BackgroundWorker;
pub use tracing_layer_core::layer::WebhookLayer;
pub use tracing_layer_core::filters::EventFilters;
use serde::Serialize;
use tracing_layer_core::layer::WebhookLayerBuilder;
use tracing_layer_core::{Config, WebhookMessage, WebhookMessageFactory, WebhookMessageInputs};

/// Layer for forwarding tracing events to Slack.
pub struct SlackLayer;

impl SlackLayer {
    pub fn builder(app_name: String, target_filters: EventFilters) -> WebhookLayerBuilder<SlackConfig, Self> {
        WebhookLayer::builder(app_name, target_filters)
    }
}

impl WebhookMessageFactory for SlackLayer {
    fn create(inputs: WebhookMessageInputs) -> impl WebhookMessage {
        let target = inputs.target;
        let span = inputs.span;
        let metadata = inputs.metadata;
        let message = inputs.message;
        let app_name = inputs.app_name;
        let source_file = inputs.source_file;
        let source_line = inputs.source_line;
        let event_level = inputs.event_level;

        #[cfg(feature = "blocks")]
        {
            let event_level_emoji = match event_level {
                tracing::Level::TRACE => ":mag:",
                tracing::Level::DEBUG => ":bug:",
                tracing::Level::INFO => ":information_source:",
                tracing::Level::WARN => ":warning:",
                tracing::Level::ERROR => ":x:",
            };
            let blocks = serde_json::json!([
                {
                    "type": "context",
                    "elements": [
                        {
                            "type": "mrkdwn",
                            "text": format!("{} - {} *{}*", app_name, event_level_emoji, event_level),
                        }
                    ]
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": format!("\"_{}_\"", message),
                    }
                },
                {
                    "type": "section",
                    "fields": [
                        {
                            "type": "mrkdwn",
                            "text": format!("*Target Span*\n{}::{}", target, span)
                        },
                        {
                            "type": "mrkdwn",
                            "text": format!("*Source*\n{}#L{}", source_file, source_line)
                        }
                    ]
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": "*Metadata:*"
                    }
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": format!("```\n{}\n```", metadata)
                    }
                }
            ]);
            let blocks_json = blocks.to_string();
            SlackMessagePayload {
                text: None,
                blocks: Some(blocks_json),
                webhook_url: inputs.webhook_url.to_string(),
            }
        }
        #[cfg(not(feature = "blocks"))]
        {
            let event_level = event.metadata().level().as_str();
            let source_file = event.metadata().file().unwrap_or("Unknown");
            let source_line = event.metadata().line().unwrap_or(0);
            let payload = format!(
                concat!(
                    "*Trace from {}*\n",
                    "*Event [{}]*: \"{}\"\n",
                    "*Target*: _{}_\n",
                    "*Span*: _{}_\n",
                    "*Metadata*:\n",
                    "```",
                    "{}",
                    "```\n",
                    "*Source*: _{}#L{}_",
                ),
                app_name, event_level, message, span, target, metadata, source_file, source_line,
            );
            SlackMessagePayload {
                text: Some(payload),
                blocks: None,
                webhook_url: webhook_url.to_string(),
            }
        }
    }
}

/// The message sent to Slack. The logged record being "drained" will be
/// converted into this format.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SlackMessagePayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<String>,
    #[serde(skip_serializing)]
    webhook_url: String,
}

impl WebhookMessage for SlackMessagePayload {
    fn webhook_url(&self) -> &str {
        self.webhook_url.as_str()
    }

    fn serialize(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize slack message")
    }
}

/// Configuration describing how to forward tracing events to Slack.
pub struct SlackConfig {
    pub(crate) webhook_url: String,
}

impl SlackConfig {
    pub fn new(webhook_url: String) -> Self {
        Self { webhook_url }
    }

    /// Create a new config for forwarding messages to Slack using configuration
    /// available in the environment.
    ///
    /// Required env vars:
    ///   * SLACK_WEBHOOK_URL
    pub fn new_from_env() -> Self {
        Self::new(std::env::var("SLACK_WEBHOOK_URL").expect("slack webhook url in env"))
    }
}

impl Default for SlackConfig {
    fn default() -> Self {
        Self::new_from_env()
    }
}

impl Config for SlackConfig {
    fn webhook_url(&self) -> &str {
        &self.webhook_url
    }

    fn new_from_env() -> Self where Self: Sized {
        Self::new_from_env()
    }
}

#[cfg(test)]
mod tests {

}

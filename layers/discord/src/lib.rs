#![doc = include_str!("../README.md")]

use serde::Serialize;
use serde_json::Value;
pub use tracing_layer_core::filters::EventFilters;
pub use tracing_layer_core::layer::WebhookLayer;
use tracing_layer_core::layer::WebhookLayerBuilder;
pub use tracing_layer_core::BackgroundWorker;
use tracing_layer_core::{Config, WebhookMessage, WebhookMessageFactory, WebhookMessageInputs};

pub struct DiscordLayer;

impl DiscordLayer {
    pub fn builder(app_name: String, target_filters: EventFilters) -> WebhookLayerBuilder<DiscordConfig, Self> {
        WebhookLayer::builder(app_name, target_filters)
    }
}

impl WebhookMessageFactory for DiscordLayer {
    fn create(inputs: WebhookMessageInputs) -> impl WebhookMessage {
        let target = inputs.target;
        let span = inputs.span;
        let metadata = inputs.metadata;
        let message = inputs.message;
        let app_name = inputs.app_name;
        let source_file = inputs.source_file;
        let source_line = inputs.source_line;
        let event_level = inputs.event_level;

        #[cfg(feature = "embed")]
        {
            let event_level_emoji = match event_level {
                tracing::Level::TRACE => ":mag:",
                tracing::Level::DEBUG => ":bug:",
                tracing::Level::INFO => ":information_source:",
                tracing::Level::WARN => ":warning:",
                tracing::Level::ERROR => ":x:",
            };
            let event_level_color = match event_level {
                tracing::Level::TRACE => 1752220,
                tracing::Level::DEBUG => 1752220,
                tracing::Level::INFO => 5763719,
                tracing::Level::WARN => 15105570,
                tracing::Level::ERROR => 15548997,
            };

            // Maximum characters allowed for a Discord field value
            const MAX_FIELD_VALUE_CHARS: usize = 1024 - 15;
            const MAX_ERROR_MESSAGE_CHARS: usize = 2048 - 15;

            // Truncate error_message if it exceeds the limit
            let mut truncated_message = String::new();
            if message.chars().count() > MAX_ERROR_MESSAGE_CHARS {
                #[cfg(feature = "log-errors")]
                eprintln!(
                    "WARN: Truncating message to {} characters, original: {}",
                    MAX_ERROR_MESSAGE_CHARS, message
                );
                let mut char_count = 0;
                for c in message.chars() {
                    char_count += 1;
                    if char_count > MAX_ERROR_MESSAGE_CHARS {
                        break;
                    }
                    truncated_message.push(c);
                }
            }
            let message = if truncated_message.is_empty() {
                message
            } else {
                truncated_message
            };

            let mut discord_embed = serde_json::json!({
                "title": format!("{} - {} {}", app_name, event_level_emoji, event_level),
                "description": format!("```rust\n{}\n```", message),
                "fields": [
                    {
                        "name": "Target Span",
                        "value": format!("`{}::{}`", target, span),
                        "inline": true
                    },
                    {
                        "name": "Source",
                        "value": format!("`{}#L{}`", source_file, source_line),
                        "inline": true
                    },
                ],
                "footer": {
                    "text": app_name
                },
                "color": event_level_color, // Hex value for "red"
                "thumbnail": {
                    "url": "https://example.com/error-thumbnail.png"
                }
            });

            // Check if metadata exceeds the limit
            if metadata.len() <= MAX_FIELD_VALUE_CHARS {
                // Metadata fits within a single field
                discord_embed["fields"].as_array_mut().unwrap().push(serde_json::json!({
                    "name": "Metadata",
                    "value": format!("```json\n{}\n```", metadata),
                    "inline": false
                }));
            } else {
                // Metadata exceeds the limit, split into multiple fields
                let mut remaining_metadata = metadata;
                let mut chunk_number = 1;
                while !remaining_metadata.is_empty() {
                    let chunk = remaining_metadata
                        .chars()
                        .take(MAX_FIELD_VALUE_CHARS)
                        .collect::<String>();

                    remaining_metadata = remaining_metadata.chars().skip(MAX_FIELD_VALUE_CHARS).collect();

                    discord_embed["fields"].as_array_mut().unwrap().push(serde_json::json!({
                        "name": format!("Metadata ({})", chunk_number),
                        "value": format!("```json\n{}\n```", chunk),
                        "inline": false
                    }));

                    chunk_number += 1;
                }
            }

            DiscordMessagePayload {
                content: None,
                embeds: Some(vec![discord_embed]),
                webhook_url: inputs.webhook_url,
            }
        }
        #[cfg(not(feature = "embed"))]
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
            DiscordMessagePayload {
                content: Some(payload),
                embeds: None,
                webhook_url: inputs.webhook_url,
            }
        }
    }
}

/// Configuration describing how to forward tracing events to Discord.
pub struct DiscordConfig {
    pub(crate) webhook_url: String,
}

impl DiscordConfig {
    pub fn new(webhook_url: String) -> Self {
        Self { webhook_url }
    }

    /// Create a new config for forwarding messages to Discord using configuration
    /// available in the environment.
    ///
    /// Required env vars:
    ///   * DISCORD_WEBHOOK_URL
    pub fn new_from_env() -> Self {
        Self::new(std::env::var("DISCORD_WEBHOOK_URL").expect("discord webhook url in env"))
    }
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self::new_from_env()
    }
}

impl Config for DiscordConfig {
    fn webhook_url(&self) -> &str {
        &self.webhook_url
    }

    fn new_from_env() -> Self
    where
        Self: Sized,
    {
        Self::new_from_env()
    }
}

/// The message sent to Discord. The logged record being "drained" will be
/// converted into this format.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DiscordMessagePayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embeds: Option<Vec<Value>>,
    #[serde(skip_serializing)]
    webhook_url: String,
}

impl WebhookMessage for DiscordMessagePayload {
    fn webhook_url(&self) -> &str {
        self.webhook_url.as_str()
    }

    fn serialize(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize discord message")
    }
}

#[cfg(test)]
mod tests {}

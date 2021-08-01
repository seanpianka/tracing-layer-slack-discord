use serde::{
    ser::{SerializeMap, Serializer},
    Serialize,
};
use serde_json::Value;
use tracing::{Event, Subscriber};
use tracing_bunyan_formatter::{JsonStorage, Type};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::SpanRef;

type ChannelSender = tokio::sync::mpsc::UnboundedSender<SlackPayload>;
type ChannelReceiver = tokio::sync::mpsc::UnboundedReceiver<SlackPayload>;

/// Configuration describing how to forward tracing events to Slack.
pub struct SlackConfig {
    channel_name: String,
    username: String,
    icon_emoji: Option<String>,
    webhook_url: String,
}

/// Layer for forwarding tracing events to Slack.
pub struct SlackForwardingLayer {
    target_filter: String,
    config: SlackConfig,
    msg_tx: ChannelSender,
}

impl SlackForwardingLayer {
    /// Create a new layer for forwarding messages to Slack, using a specified configuration.
    pub fn new(target_filter: String, config: SlackConfig, sender: ChannelSender) -> Self {
        Self {
            target_filter,
            config,
            msg_tx: sender,
        }
    }

    /// Create a new layer for forwarding messages to Slack, using configuration available in the environment.
    ///
    /// Required env vars:
    ///   * SLACK_WEBHOOK_URL
    ///   * SLACK_CHANNEL_NAME
    ///   * SLACK_USERNAME
    ///
    /// Optional env vars:
    ///   * SLACK_EMOJI
    pub fn new_from_env(target_filter: String, sender: ChannelSender) -> Self {
        Self::new(target_filter, SlackConfig::default(), sender)
    }
}

impl SlackConfig {
    const DEFAULT_EMOJI: &'static str = "robot";

    pub fn new(webhook_url: String, channel_name: String, username: String, icon_emoji: Option<String>) -> Self {
        Self {
            channel_name,
            username,
            icon_emoji,
            webhook_url,
        }
    }
}

impl Default for SlackConfig {
    fn default() -> Self {
        Self::new(
            std::env::var("SLACK_WEBHOOK_URL").expect("slack webhook url in env"),
            std::env::var("SLACK_CHANNEL_NAME").expect("slack channel name in env"),
            std::env::var("SLACK_USERNAME").expect("slack username in env"),
            std::env::var("SLACK_EMOJI").ok().or_else(|| Some(String::from(Self::DEFAULT_EMOJI))),
        )
    }
}

/// The message sent to Slack. The logged record being "drained" will be
/// converted into this format.
#[derive(Debug, Clone, Serialize)]
pub struct SlackPayload {
    channel: String,
    username: String,
    text: String,
    #[serde(skip_serializing)]
    webhook_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_emoji: Option<String>,
}

impl SlackPayload {
    pub fn webhook_url(&self) -> &str {
        self.webhook_url.as_str()
    }
}

/// Provides a background worker task that sends the messages generated by the layer.
pub async fn worker(mut rx: ChannelReceiver) {
    let client = reqwest::Client::new();
    while let Some(payload) = rx.recv().await {
        let webhook_url = payload.webhook_url().clone();
        let payload = serde_json::to_string(&payload).expect("failed to deserialize slack payload, this is a bug");
        match client.post(webhook_url).body(payload).send().await {
            Ok(res) => {
                println!("{}", res.text().await.unwrap());
            }
            Err(e) => {
                println!("{}", e);
            }
        };
    }
}

/// Ensure consistent formatting of the span context.
///
/// Example: "[AN_INTERESTING_SPAN - START]" is how it'd look

fn format_span_context<S>(
    span: &SpanRef<S>,
    ty: Type,
) -> String where S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a> {
    format!("[{} - {}]", span.metadata().name().to_uppercase(), ty)
}

impl<S> Layer<S> for SlackForwardingLayer
    where S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>, {
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let current_span = ctx.lookup_current();

        let mut event_visitor = JsonStorage::default();
        event.record(&mut event_visitor);

        let format = || {
            let mut buffer = Vec::new();

            let mut serializer = serde_json::Serializer::new(&mut buffer);
            let mut map_serializer = serializer.serialize_map(None)?;

            // Extract the "message" field, if provided. Fallback to the target, if missing.
            let mut message = event_visitor.values().get("message").map(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            }).flatten().unwrap_or_else(|| event.metadata().target()).to_owned();

            // If the event is in the context of a span, prepend the span name to the message.
            if let Some(span) = &current_span {
                message = format!("{} {}", format_span_context(span, Type::Event), message);
            }

            map_serializer.serialize_entry("msg", &message)?;

            // Additional metadata useful for debugging
            // They should be nested under `src` (see https://github.com/trentm/node-bunyan#src )
            // but `tracing` does not support nested values yet
            let target = event.metadata().target();
            if target != self.target_filter {
                return Err(std::io::Error::from_raw_os_error(1));
            }
            map_serializer.serialize_entry("target", event.metadata().target())?;
            map_serializer.serialize_entry("line", &event.metadata().line())?;
            map_serializer.serialize_entry("file", &event.metadata().file())?;

            // Add all the other fields associated with the event, expect the message we already used.
            for (key, value) in event_visitor.values().iter().filter(|(&key, _)| key != "message") {
                map_serializer.serialize_entry(key, value)?;
            }

            // Add all the fields from the current span, if we have one.
            if let Some(span) = &current_span {
                let extensions = span.extensions();
                if let Some(visitor) = extensions.get::<JsonStorage>() {
                    for (key, value) in visitor.values() {
                        map_serializer.serialize_entry(key, value)?;
                    }
                }
            }
            map_serializer.end()?;
            Ok(buffer)
        };

        let result: std::io::Result<Vec<u8>> = format();
        if let Ok(formatted) = result {
            let text = String::from_utf8(formatted.clone()).unwrap();
            println!("{}", text.as_str());
            let payload = SlackPayload {
                channel: self.config.channel_name.clone(),
                username: self.config.username.clone(),
                text,
                webhook_url: self.config.webhook_url.clone(),
                icon_emoji: self.config.icon_emoji.clone(),
            };
            if let Err(e) = self.msg_tx.send(payload) {
                tracing::error!(err = %e, "failed to send slack payload to given channel")
            };
        }
    }
}
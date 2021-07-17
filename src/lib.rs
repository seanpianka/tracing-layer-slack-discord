use std::sync::Arc;

use serde::{
    ser::{SerializeMap, Serializer},
    Serialize,
};
use serde_json::Value;
use tracing::{Event, Subscriber};
use tracing_bunyan_formatter::{JsonStorage, Type};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::SpanRef;
use tracing_subscriber::Layer;

/// The message sent to Slack. The logged record being "drained" will be
/// converted into this format.
#[derive(Debug, Clone, Serialize)]
struct SlackPayload {
    channel: String,
    username: String,
    text: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_emoji: Option<String>,
}

pub struct SlackForwardingLayer {
    channel_name: String,
    username: String,
    icon_emoji: Option<String>,
    webhook_url: String,

    client: Arc<reqwest::Client>,
}

impl SlackForwardingLayer {
    pub fn new(webhook_url: String, channel_name: String, username: String, icon_emoji: Option<String>) -> Self {
        let client = {
            let c = reqwest::Client::builder().build().expect("failed to build HTTP client");
            Arc::new(c)
        };
        Self {
            channel_name,
            username,
            icon_emoji,
            webhook_url,
            client,
        }
    }
}

async fn post(client: Arc<reqwest::Client>, webhook_url: String, payload: SlackPayload) {
    let payload = serde_json::to_string(&payload).expect("failed to deserialize slack payload, this is a bug");
    match client.post(webhook_url).body(payload).send().await {
        Ok(res) => {
            tracing::debug!("{:?}", res);
        }
        Err(e) => {
            tracing::error!(err = ?e);
        }
    };
}

/// Ensure consistent formatting of the span context.
///
/// Example: "[AN_INTERESTING_SPAN - START]"
fn format_span_context<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>>(
    span: &SpanRef<S>,
    ty: Type,
) -> String {
    format!("[{} - {}]", span.metadata().name().to_uppercase(), ty)
}

impl<S> Layer<S> for SlackForwardingLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let current_span = ctx.lookup_current();

        let mut event_visitor = JsonStorage::default();
        event.record(&mut event_visitor);

        let format = || {
            let mut buffer = Vec::new();

            let mut serializer = serde_json::Serializer::new(&mut buffer);
            let mut map_serializer = serializer.serialize_map(None)?;

            // Extract the "message" field, if provided. Fallback to the target, if missing.
            let mut message = event_visitor
                .values()
                .get("message")
                .map(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .flatten()
                .unwrap_or_else(|| event.metadata().target())
                .to_owned();

            // If the event is in the context of a span, prepend the span name to the message.
            if let Some(span) = &current_span {
                message = format!("{} {}", format_span_context(span, Type::Event), message);
            }

            map_serializer.serialize_entry("msg", &message)?;

            // Additional metadata useful for debugging
            // They should be nested under `src` (see https://github.com/trentm/node-bunyan#src )
            // but `tracing` does not support nested values yet
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
            let payload = SlackPayload {
                channel: self.channel_name.clone(),
                username: self.username.clone(),
                text: formatted,
                icon_emoji: self.icon_emoji.clone(),
            };
            let webhook_url = self.webhook_url.clone();
            let client = self.client.clone();
            tokio::spawn(async { post(client, webhook_url, payload).await });
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_ne!(2 + 3, 4);
    }
}

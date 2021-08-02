use std::future::Future;

use serde::ser::{SerializeMap, Serializer};
use serde_json::Value;
use tracing::{Event, Subscriber};
use tracing_bunyan_formatter::{JsonStorage, Type};
use tracing_subscriber::{layer::Context, registry::SpanRef, Layer};

use crate::matches::{EventFilters, Matcher};
use crate::worker::WorkerMessage;
use crate::{config::SlackConfig, message::SlackPayload, worker::worker, ChannelSender};

/// Layer for forwarding tracing events to Slack.
pub struct SlackForwardingLayer {
    /// Include/exclude event records based on filters against the event's target.
    target_regex_filters: Option<EventFilters>,

    /// Include/exclude event records based on filters against the event's message.
    message_regex_filters: Option<EventFilters>,

    /// Include/exclude event records based on whether an event contains a certain field.
    event_field_filters: Option<EventFilters>,

    /// Configure the layer's connection to the Slack Webhook API.
    config: SlackConfig,

    /// `tracing-layer-slack` synchronously generates payloads to send to the Slack API using the
    /// tracing events from the global subscriber. However, all network requests are offloaded onto
    /// an unbuffered channel and processed by a provided future acting as an asynchronous worker.
    ///
    /// An unbounded sender, which the caller must send `WorkerMessage::Shutdown` in order to cancel
    /// the message receive loop
    shutdown_sender: ChannelSender,
}

impl SlackForwardingLayer {
    /// Create a new layer for forwarding messages to Slack, using a specified
    /// configuration.
    ///
    /// Returns the tracing_subscriber::Layer impl to add to a registry, an unbounded-mpsc sender
    /// used to shutdown the background worker, and a future to spawn as a task on a tokio runtime
    /// to initialize the worker's processing and sending of HTTP requests to the Slack API.
    pub fn new(
        target_regex_filters: Option<EventFilters>,
        message_regex_filters: Option<EventFilters>,
        event_field_filters: Option<EventFilters>,
        config: SlackConfig,
    ) -> (SlackForwardingLayer, ChannelSender, impl Future<Output = ()>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let layer = SlackForwardingLayer {
            target_regex_filters,
            message_regex_filters,
            event_field_filters,
            config,
            shutdown_sender: tx.clone(),
        };
        (layer, tx, worker(rx))
    }

    /// Create a new layer for forwarding messages to Slack, using configuration
    /// available in the environment.
    ///
    /// Required env vars:
    ///   * SLACK_WEBHOOK_URL
    ///   * SLACK_CHANNEL_NAME
    ///   * SLACK_USERNAME
    ///
    /// Optional env vars:
    ///   * SLACK_EMOJI
    pub fn new_from_env(
        target_filters: Option<EventFilters>,
        message_filters: Option<EventFilters>,
        event_filters: Option<EventFilters>,
    ) -> (SlackForwardingLayer, ChannelSender, impl Future<Output = ()>) {
        Self::new(target_filters, message_filters, event_filters, SlackConfig::default())
    }
}

/// Ensure consistent formatting of the span context.
///
/// Example: "[AN_INTERESTING_SPAN - START]" is how it'd look

fn format_span_context<S>(span: &SpanRef<S>, ty: Type) -> String
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
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
            self.message_regex_filters.process(message.as_str())?;
            // If the event is in the context of a span, prepend the span name to the
            // message.
            if let Some(span) = &current_span {
                message = format!("{} {}", format_span_context(span, Type::Event), message);
            }
            map_serializer.serialize_entry("message", &message)?;

            // Additional metadata useful for debugging
            // They should be nested under `src` (see https://github.com/trentm/node-bunyan#src )
            // but `tracing` does not support nested values yet
            let target = event.metadata().target();
            self.target_regex_filters.process(target)?;
            map_serializer.serialize_entry("target", event.metadata().target())?;

            map_serializer.serialize_entry("line", &event.metadata().line())?;
            map_serializer.serialize_entry("file", &event.metadata().file())?;
            // Add all the other fields associated with the event, expect the message we
            // already used.
            for (key, value) in event_visitor
                .values()
                .iter()
                .filter(|(&key, _)| key != "message")
                .filter(|(&key, _)| self.event_field_filters.process(key).is_ok())
            {
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

        let result: Result<Vec<u8>, crate::matches::MatchingError> = format();
        if let Ok(formatted) = result {
            let text = String::from_utf8(formatted.clone()).unwrap();
            println!("{}", text.as_str());
            let payload = SlackPayload::new(
                self.config.channel_name.clone(),
                self.config.username.clone(),
                text,
                self.config.webhook_url.clone(),
                self.config.icon_emoji.clone(),
            );
            if let Err(e) = self.shutdown_sender.send(WorkerMessage::Data(payload)) {
                tracing::error!(err = %e, "failed to send slack payload to given channel")
            };
        }
    }
}

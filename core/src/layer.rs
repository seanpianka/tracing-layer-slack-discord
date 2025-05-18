use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use regex::Regex;
use serde::ser::SerializeMap;
use serde::Serializer;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::log::LevelFilter;
use tracing::{Event, Subscriber};
use tracing_bunyan_formatter::JsonStorage;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use crate::filters::{Filter, FilterError};
use crate::{
    BackgroundWorker, ChannelSender, EventFilters, WebhookMessageFactory, WebhookMessageInputs, WorkerMessage,
};

/// Layer for forwarding tracing events to webhook endpoints.
pub struct WebhookLayer<F: WebhookMessageFactory> {
    factory: F,

    app_name: String,

    /// Filter events by their target.
    ///
    /// Filter type semantics:
    /// - Subtractive: Exclude an event if the target does NOT MATCH a given regex.
    /// - Additive: Exclude an event if the target MATCHES a given regex.
    target_filters: EventFilters,

    /// Filter events by their message.
    ///
    /// Filter type semantics:
    /// - Positive: Exclude an event if the message MATCHES a given regex, and
    /// - Negative: Exclude an event if the message does NOT MATCH a given regex.
    message_filters: Option<EventFilters>,

    /// Filter events by fields.
    ///
    /// Filter type semantics:
    /// - Positive: Exclude the event if its key MATCHES a given regex.
    /// - Negative: Exclude the event if its key does NOT MATCH a given regex.
    event_by_field_filters: Option<EventFilters>,

    /// Filter fields of events from being sent to the webhook.
    ///
    /// Filter type semantics:
    /// - Positive: Exclude event fields if the field's key MATCHES any provided regular expressions.
    field_exclusion_filters: Option<Vec<Regex>>,

    /// Filter events by their level.
    level_filter: Option<String>,

    /// An unbounded sender, which the caller must send `WorkerMessage::Shutdown` in order to cancel
    /// worker's receive-send loop.
    sender: ChannelSender,
}

impl<F: WebhookMessageFactory> WebhookLayer<F> {
    /// Create a new layer for forwarding messages to the webhook, using a specified
    /// configuration. The background worker must be started in order to spawn spawns
    /// a task onto the tokio runtime to begin sending tracing events to the webhook.
    ///
    /// Returns the tracing_subscriber::Layer impl to add to a registry, an unbounded-mpsc sender
    /// used to shutdown the background worker, and a future to spawn as a task on a tokio runtime
    /// to initialize the worker's processing and sending of HTTP requests to the webhook.
    pub(crate) fn new(
        factory: F,
        app_name: String,
        target_filters: EventFilters,
        message_filters: Option<EventFilters>,
        event_by_field_filters: Option<EventFilters>,
        field_exclusion_filters: Option<Vec<Regex>>,
        level_filter: Option<String>,
    ) -> (WebhookLayer<F>, BackgroundWorker) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let layer = WebhookLayer {
            factory,
            app_name,
            target_filters,
            message_filters,
            event_by_field_filters,
            field_exclusion_filters,
            level_filter,
            sender: tx.clone(),
        };
        let background_worker = BackgroundWorker {
            sender: tx,
            handle: Arc::new(Mutex::new(None)),
            rx: Arc::new(Mutex::new(rx)),
        };
        (layer, background_worker)
    }
}

/// A builder for creating a webhook layer.
///
/// The layer requires a regex for selecting events to be sent to webhook by their target. Specifying
/// no filter (e.g. ".*") will cause an explosion in the number of messages observed by the layer.
///
/// Several methods expose initialization of optional filtering mechanisms.
pub struct WebhookLayerBuilder<F: WebhookMessageFactory> {
    factory: F,
    app_name: String,
    target_filters: EventFilters,
    message_filters: Option<EventFilters>,
    event_by_field_filters: Option<EventFilters>,
    field_exclusion_filters: Option<Vec<Regex>>,
    level_filters: Option<String>,
}
impl<F: WebhookMessageFactory> WebhookLayerBuilder<F> {
    pub fn new(factory: F, app_name: String, target_filters: EventFilters) -> Self {
        Self {
            factory,
            app_name,
            target_filters,
            message_filters: None,
            event_by_field_filters: None,
            field_exclusion_filters: None,
            level_filters: None,
        }
    }

    /// Filter events by their message.
    ///
    /// Filter type semantics:
    /// - Positive: Exclude an event if the message MATCHES a given regex, and
    /// - Negative: Exclude an event if the message does NOT MATCH a given regex.
    pub fn message_filters(mut self, filters: EventFilters) -> Self {
        self.message_filters = Some(filters);
        self
    }

    /// Filter events by fields.
    ///
    /// Filter type semantics:
    /// - Positive: Exclude the event if its key MATCHES a given regex.
    /// - Negative: Exclude the event if its key does NOT MATCH a given regex.
    pub fn event_by_field_filters(mut self, filters: EventFilters) -> Self {
        self.event_by_field_filters = Some(filters);
        self
    }

    /// Filter fields of events from being sent to the webhook.
    ///
    /// Filter type semantics:
    /// - Positive: Exclude event fields if the field's key MATCHES any provided regular expressions.
    pub fn field_exclusion_filters(mut self, filters: Vec<Regex>) -> Self {
        self.field_exclusion_filters = Some(filters);
        self
    }

    /// Configure which levels of events to send to the webhook.
    pub fn level_filters(mut self, level_filters: String) -> Self {
        self.level_filters = Some(level_filters);
        self
    }

    /// Create a webhook layer and its corresponding background worker to (async) send the messages.
    pub fn build(self) -> (WebhookLayer<F>, BackgroundWorker) {
        WebhookLayer::new(
            self.factory,
            self.app_name,
            self.target_filters,
            self.message_filters,
            self.event_by_field_filters,
            self.field_exclusion_filters,
            self.level_filters,
        )
    }
}

impl<S, F> Layer<S> for WebhookLayer<F>
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    F: WebhookMessageFactory + 'static,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let current_span = ctx.lookup_current();
        let mut event_visitor = JsonStorage::default();
        event.record(&mut event_visitor);

        let format = || {
            const KEYWORDS: [&str; 2] = ["message", "error"];

            let target = event.metadata().target();
            self.target_filters.process(target)?;

            // Extract the "message" field, if provided. Fallback to the target, if missing.
            let message = event_visitor
                .values()
                .get("message")
                .and_then(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .or_else(|| {
                    event_visitor.values().get("error").and_then(|v| match v {
                        Value::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                })
                .unwrap_or("No message");

            self.message_filters.process(message)?;
            if let Some(level_filters) = &self.level_filter {
                let message_level = {
                    LevelFilter::from_str(event.metadata().level().as_str())
                        .map_err(|e| FilterError::IoError(Box::new(e)))?
                };
                let level_threshold =
                    LevelFilter::from_str(level_filters).map_err(|e| FilterError::IoError(Box::new(e)))?;
                if message_level > level_threshold {
                    return Err(FilterError::PositiveFilterFailed);
                }
            }

            let mut metadata_buffer = Vec::new();
            let mut serializer = serde_json::Serializer::new(&mut metadata_buffer);
            let mut map_serializer = serializer.serialize_map(None)?;
            // Add all the other fields associated with the event, expect the message we
            // already used.
            for (key, value) in event_visitor
                .values()
                .iter()
                .filter(|(&key, _)| !KEYWORDS.contains(&key))
                .filter(|(&key, _)| self.field_exclusion_filters.process(key).is_ok())
            {
                self.event_by_field_filters.process(key)?;
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

            let span = match &current_span {
                Some(span) => span.metadata().name(),
                None => "",
            };

            let metadata = {
                let data: HashMap<String, Value> = serde_json::from_slice(metadata_buffer.as_slice()).unwrap();
                serde_json::to_string_pretty(&data).unwrap()
            };

            let message = self.factory.create(WebhookMessageInputs {
                app_name: self.app_name.clone(),
                message: message.to_string(),
                event_level: *event.metadata().level(),
                source_file: event.metadata().file().unwrap_or("Unknown").to_string(),
                source_line: event.metadata().line().unwrap_or(0),
                target: target.to_string(),
                span: span.to_string(),
                metadata,
            });

            Ok(message)
        };

        let result: Result<_, FilterError> = format();
        if let Ok(formatted) = result {
            if let Err(e) = self.sender.send(WorkerMessage::Data(formatted)) {
                #[cfg(feature = "log-errors")]
                eprintln!("ERROR: failed to send webhook payload to given channel, err = {}", e)
            };
        }
    }
}

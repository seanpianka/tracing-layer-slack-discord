use regex::Regex;
use serde::ser::{SerializeMap, Serializer};
use serde_json::Value;
use tracing::{Event, Subscriber};
use tracing_bunyan_formatter::JsonStorage;
use tracing_subscriber::{layer::Context, Layer};

use crate::filters::{EventFilters, Filter, FilterError};
use crate::message::PayloadMessageType;
use crate::worker::{SlackBackgroundWorker, WorkerMessage};
use crate::{config::SlackConfig, message::SlackPayload, worker::worker, ChannelSender};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::log::LevelFilter;

/// Layer for forwarding tracing events to Slack.
pub struct SlackLayer {
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

    /// Filter fields of events from being sent to Slack.
    ///
    /// Filter type semantics:
    /// - Positive: Exclude event fields if the field's key MATCHES any provided regular expressions.
    field_exclusion_filters: Option<Vec<Regex>>,

    /// Filter events by their level.
    level_filter: Option<String>,

    #[cfg(feature = "blocks")]
    app_name: String,

    /// Configure the layer's connection to the Slack Webhook API.
    config: SlackConfig,

    /// An unbounded sender, which the caller must send `WorkerMessage::Shutdown` in order to cancel
    /// worker's receive-send loop.
    slack_sender: ChannelSender,
}

impl SlackLayer {
    /// Create a new layer for forwarding messages to Slack, using a specified
    /// configuration. This method spawns a task onto the tokio runtime to begin sending tracing
    /// events to Slack.
    ///
    /// Returns the tracing_subscriber::Layer impl to add to a registry, an unbounded-mpsc sender
    /// used to shutdown the background worker, and a future to spawn as a task on a tokio runtime
    /// to initialize the worker's processing and sending of HTTP requests to the Slack API.
    pub(crate) fn new(
        app_name: String,
        target_filters: EventFilters,
        message_filters: Option<EventFilters>,
        event_by_field_filters: Option<EventFilters>,
        field_exclusion_filters: Option<Vec<Regex>>,
        level_filter: Option<String>,
        config: SlackConfig,
    ) -> (SlackLayer, SlackBackgroundWorker) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let layer = SlackLayer {
            target_filters,
            message_filters,
            field_exclusion_filters,
            event_by_field_filters,
            level_filter,
            app_name,
            config,
            slack_sender: tx.clone(),
        };
        let worker = SlackBackgroundWorker {
            sender: tx,
            handle: Arc::new(Mutex::new(Some(tokio::spawn(worker(rx))))),
        };
        (layer, worker)
    }

    /// Create a new builder for SlackLayer.
    pub fn builder(app_name: String, target_filters: EventFilters) -> SlackLayerBuilder {
        SlackLayerBuilder::new(app_name, target_filters)
    }
}

/// A builder for creating a Slack layer.
///
/// The layer requires a regex for selecting events to be sent to Slack by their target. Specifying
/// no filter (e.g. ".*") will cause an explosion in the number of messages observed by the layer.
///
/// Several methods expose initialization of optional filtering mechanisms, along with Slack
/// configuration that defaults to searching in the local environment variables.
pub struct SlackLayerBuilder {
    app_name: String,
    target_filters: EventFilters,
    message_filters: Option<EventFilters>,
    event_by_field_filters: Option<EventFilters>,
    field_exclusion_filters: Option<Vec<Regex>>,
    level_filters: Option<String>,
    config: Option<SlackConfig>,
}

impl SlackLayerBuilder {
    pub(crate) fn new(app_name: String, target_filters: EventFilters) -> Self {
        Self {
            app_name,
            target_filters,
            message_filters: None,
            event_by_field_filters: None,
            field_exclusion_filters: None,
            level_filters: None,
            config: None,
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

    /// Filter fields of events from being sent to Slack.
    ///
    /// Filter type semantics:
    /// - Positive: Exclude event fields if the field's key MATCHES any provided regular expressions.
    pub fn field_exclusion_filters(mut self, filters: Vec<Regex>) -> Self {
        self.field_exclusion_filters = Some(filters);
        self
    }

    /// Configure the layer's connection to the Slack Webhook API.
    pub fn slack_config(mut self, config: SlackConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Configure which levels of events to send to Slack.
    pub fn level_filters(mut self, level_filters: String) -> Self {
        self.level_filters = Some(level_filters);
        self
    }

    /// Create a SlackLayer and its corresponding background worker to (async) send the messages.
    pub fn build(self) -> (SlackLayer, SlackBackgroundWorker) {
        SlackLayer::new(
            self.app_name,
            self.target_filters,
            self.message_filters,
            self.event_by_field_filters,
            self.field_exclusion_filters,
            self.level_filters,
            self.config.unwrap_or_else(SlackConfig::new_from_env),
        )
    }
}

impl<S> Layer<S> for SlackLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
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
                let data: HashMap<String, serde_json::Value> =
                    serde_json::from_slice(metadata_buffer.as_slice()).unwrap();
                serde_json::to_string_pretty(&data).unwrap()
            };

            Ok(Self::format_payload(
                self.app_name.as_str(),
                message,
                event,
                target,
                span,
                metadata,
            ))
        };

        let result: Result<PayloadMessageType, crate::filters::FilterError> = format();
        if let Ok(formatted) = result {
            let payload = SlackPayload::new(formatted, self.config.webhook_url.clone());
            if let Err(e) = self.slack_sender.send(WorkerMessage::Data(payload)) {
                tracing::error!(err = %e, "failed to send slack payload to given channel")
            };
        }
    }
}

impl SlackLayer {
    #[cfg(feature = "blocks")]
    fn format_payload(
        app_name: &str,
        message: &str,
        event: &Event,
        target: &str,
        span: &str,
        metadata: String,
    ) -> PayloadMessageType {
        let event_level = event.metadata().level();
        let event_level_emoji = match *event_level {
            tracing::Level::TRACE => ":mag:",
            tracing::Level::DEBUG => ":bug:",
            tracing::Level::INFO => ":information_source:",
            tracing::Level::WARN => ":warning:",
            tracing::Level::ERROR => ":x:",
        };
        let source_file = event.metadata().file().unwrap_or("Unknown");
        let source_line = event.metadata().line().unwrap_or(0);
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
        PayloadMessageType::Blocks(blocks_json)
    }

    #[cfg(not(feature = "blocks"))]
    fn format_payload(
        app_name: &str,
        message: &str,
        event: &Event,
        target: &str,
        span: &str,
        metadata: String,
    ) -> PayloadMessageType {
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
        PayloadMessageType::Text(payload)
    }
}

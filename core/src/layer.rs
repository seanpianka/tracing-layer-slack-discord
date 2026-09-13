use std::str::FromStr;

use regex::Regex;
use serde_json::{Map, Value};
use tracing::log::LevelFilter;
use tracing::{Event, Level, Subscriber};
use tracing_bunyan_formatter::JsonStorage;
use tracing_subscriber::layer::Context;

use crate::filters::Filter;
use crate::EventFilters;

/// A filtered, normalized Trace Event ready for platform rendering.
#[derive(Clone, Debug)]
pub struct PreparedNotification {
    app_name: String,
    message: String,
    target: String,
    span: String,
    metadata: Map<String, Value>,
    source_file: String,
    source_line: u32,
    level: Level,
}

impl PreparedNotification {
    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn span(&self) -> &str {
        &self.span
    }

    pub fn metadata(&self) -> &Map<String, Value> {
        &self.metadata
    }

    pub fn source_file(&self) -> &str {
        &self.source_file
    }

    pub fn source_line(&self) -> u32 {
        self.source_line
    }

    pub fn level(&self) -> Level {
        self.level
    }
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct PreparationConfig {
    target_filters: EventFilters,
    message_filters: Option<EventFilters>,
    event_by_field_filters: Option<EventFilters>,
    field_exclusion_filters: Option<Vec<Regex>>,
    level_filter: Option<String>,
}

impl PreparationConfig {
    pub fn new(
        target_filters: EventFilters,
        message_filters: Option<EventFilters>,
        event_by_field_filters: Option<EventFilters>,
        field_exclusion_filters: Option<Vec<Regex>>,
        level_filter: Option<String>,
    ) -> Self {
        Self {
            target_filters,
            message_filters,
            event_by_field_filters,
            field_exclusion_filters,
            level_filter,
        }
    }
}

#[doc(hidden)]
pub fn prepare_event<S>(
    app_name: &str,
    config: &PreparationConfig,
    event: &Event<'_>,
    ctx: Context<'_, S>,
) -> Option<PreparedNotification>
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    const MESSAGE_FIELDS: [&str; 2] = ["message", "error"];

    let mut event_visitor = JsonStorage::default();
    event.record(&mut event_visitor);

    let target = event.metadata().target();
    config.target_filters.process(target).ok()?;

    let message = event_visitor
        .values()
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| event_visitor.values().get("error").and_then(Value::as_str))
        .unwrap_or("No message");
    config.message_filters.process(message).ok()?;

    if let Some(level_filter) = &config.level_filter {
        let message_level = LevelFilter::from_str(event.metadata().level().as_str()).ok()?;
        let threshold = LevelFilter::from_str(level_filter).ok()?;
        if message_level > threshold {
            return None;
        }
    }

    let mut metadata = Map::new();
    for (key, value) in event_visitor.values() {
        if MESSAGE_FIELDS.contains(key) || config.field_exclusion_filters.process(key).is_err() {
            continue;
        }
        config.event_by_field_filters.process(key).ok()?;
        metadata.insert((*key).to_owned(), value.clone());
    }

    let current_span = ctx.lookup_current();
    if let Some(span) = &current_span {
        let extensions = span.extensions();
        if let Some(visitor) = extensions.get::<JsonStorage>() {
            for (key, value) in visitor.values() {
                metadata.insert((*key).to_owned(), value.clone());
            }
        }
    }

    Some(PreparedNotification {
        app_name: app_name.to_owned(),
        message: message.to_owned(),
        target: target.to_owned(),
        span: current_span.map_or_else(String::new, |span| span.metadata().name().to_owned()),
        metadata,
        source_file: event.metadata().file().unwrap_or("Unknown").to_owned(),
        source_line: event.metadata().line().unwrap_or(0),
        level: *event.metadata().level(),
    })
}

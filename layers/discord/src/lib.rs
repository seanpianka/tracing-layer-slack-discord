#![doc = include_str!("../README.md")]

use std::str::FromStr;
use std::sync::Arc;

use serde_json::{json, Value};
use tracing::{Event, Level, Subscriber};
use tracing_layer_core::private::{delivery_channel, prepare_event, Enqueue, PreparationConfig};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

pub use tracing_layer_core::{
    Delivery, DeliveryFailure, DeliveryFailureReason, DeliveryHandle, DeliveryReport, Error, EventFilters,
    PreparedNotification, WebhookUrl,
};

/// A layer that turns selected Trace Events into Discord messages.
pub struct DiscordLayer {
    app_name: String,
    preparation: PreparationConfig,
    rendering: DiscordRendering,
    mention_target: Option<MentionTarget>,
    enqueue: Enqueue,
}

impl DiscordLayer {
    pub fn builder(
        app_name: impl Into<String>,
        target_filters: EventFilters,
        webhook_url: impl AsRef<str>,
    ) -> Result<DiscordLayerBuilder, Error> {
        Ok(DiscordLayerBuilder::new(
            app_name.into(),
            target_filters,
            WebhookUrl::parse(webhook_url)?,
        ))
    }

    pub fn from_env(app_name: impl Into<String>, target_filters: EventFilters) -> Result<DiscordLayerBuilder, Error> {
        let webhook_url = std::env::var("DISCORD_WEBHOOK_URL")
            .map_err(|_| Error::MissingEnvironmentVariable("DISCORD_WEBHOOK_URL"))?;
        Self::builder(app_name, target_filters, webhook_url)
    }
}

impl<S> Layer<S> for DiscordLayer
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let Some(notification) = prepare_event(&self.app_name, &self.preparation, event, ctx) else {
            return;
        };
        let message = match self.rendering.render(&notification) {
            Ok(message) => message,
            Err(_error) => {
                #[cfg(feature = "log-errors")]
                eprintln!("ERROR: failed to render Discord notification: {_error}");
                return;
            }
        };
        let body = match apply_notification_policy(message, notification.level(), self.mention_target) {
            Ok(body) => body,
            Err(_error) => {
                #[cfg(feature = "log-errors")]
                eprintln!("ERROR: failed to apply Discord notification policy: {_error}");
                return;
            }
        };
        if let Err(_error) = self.enqueue.send(body) {
            #[cfg(feature = "log-errors")]
            eprintln!("ERROR: failed to enqueue Discord notification: {_error}");
        }
    }
}

/// Builds a Discord layer and its independently started Webhook Delivery.
pub struct DiscordLayerBuilder {
    app_name: String,
    target_filters: EventFilters,
    message_filters: Option<EventFilters>,
    event_by_field_filters: Option<EventFilters>,
    field_exclusion_filters: Option<Vec<regex::Regex>>,
    level_filter: Option<String>,
    webhook_url: WebhookUrl,
    rendering: DiscordRendering,
    mention_target: Option<MentionTarget>,
}

impl DiscordLayerBuilder {
    fn new(app_name: String, target_filters: EventFilters, webhook_url: WebhookUrl) -> Self {
        Self {
            app_name,
            target_filters,
            message_filters: None,
            event_by_field_filters: None,
            field_exclusion_filters: None,
            level_filter: None,
            webhook_url,
            rendering: DiscordRendering::BuiltIn(DiscordPresentation::Rich),
            mention_target: None,
        }
    }

    pub fn message_filters(mut self, filters: EventFilters) -> Self {
        self.message_filters = Some(filters);
        self
    }

    pub fn event_by_field_filters(mut self, filters: EventFilters) -> Self {
        self.event_by_field_filters = Some(filters);
        self
    }

    pub fn field_exclusion_filters(mut self, filters: Vec<regex::Regex>) -> Self {
        self.field_exclusion_filters = Some(filters);
        self
    }

    pub fn level_filter(mut self, level_filter: impl Into<String>) -> Self {
        self.level_filter = Some(level_filter.into());
        self
    }

    pub fn presentation(mut self, presentation: DiscordPresentation) -> Self {
        self.rendering = DiscordRendering::BuiltIn(presentation);
        self
    }

    pub fn renderer(mut self, renderer: impl DiscordRenderer) -> Self {
        self.rendering = DiscordRendering::Custom(Arc::new(renderer));
        self
    }

    pub fn mention_target(mut self, mention_target: MentionTarget) -> Self {
        self.mention_target = Some(mention_target);
        self
    }

    pub fn build(self) -> (DiscordLayer, Delivery) {
        let (enqueue, delivery) = delivery_channel(self.webhook_url);
        let preparation = PreparationConfig::new(
            self.target_filters,
            self.message_filters,
            self.event_by_field_filters,
            self.field_exclusion_filters,
            self.level_filter,
        );
        (
            DiscordLayer {
                app_name: self.app_name,
                preparation,
                rendering: self.rendering,
                mention_target: self.mention_target,
                enqueue,
            },
            delivery,
        )
    }
}

/// The built-in Discord presentation selected at runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscordPresentation {
    Rich,
    Text,
}

/// The Discord audience requested for error notifications.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MentionTarget {
    Everyone,
    Here,
    Role(DiscordRoleId),
}

/// A validated Discord role snowflake.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscordRoleId(u64);

impl DiscordRoleId {
    pub fn new(value: u64) -> Result<Self, Error> {
        if value == 0 {
            return Err(Error::InvalidConfiguration("Discord role ID must be non-zero"));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl FromStr for DiscordRoleId {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .parse::<u64>()
            .map_err(|_| Error::InvalidConfiguration("Discord role ID must be numeric"))
            .and_then(Self::new)
    }
}

/// A validated Discord webhook message without a destination.
#[derive(Clone, Debug)]
pub struct DiscordMessage(Value);

impl DiscordMessage {
    pub fn from_value(value: Value) -> Result<Self, Error> {
        validate_message(&value)?;
        Ok(Self(value))
    }

    pub fn text(content: impl Into<String>) -> Result<Self, Error> {
        Self::from_value(json!({ "content": content.into() }))
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

/// Custom Discord message creation from a read-only Prepared Notification.
pub trait DiscordRenderer: Send + Sync + 'static {
    fn render(&self, notification: &PreparedNotification) -> Result<DiscordMessage, Error>;
}

enum DiscordRendering {
    BuiltIn(DiscordPresentation),
    Custom(Arc<dyn DiscordRenderer>),
}

impl DiscordRendering {
    fn render(&self, notification: &PreparedNotification) -> Result<DiscordMessage, Error> {
        match self {
            Self::BuiltIn(DiscordPresentation::Rich) => render_rich(notification),
            Self::BuiltIn(DiscordPresentation::Text) => render_text(notification),
            Self::Custom(renderer) => renderer.render(notification),
        }
    }
}

fn render_rich(notification: &PreparedNotification) -> Result<DiscordMessage, Error> {
    const MAX_FIELD_VALUE_CHARS: usize = 1009;
    const MAX_MESSAGE_CHARS: usize = 2033;

    let emoji = match notification.level() {
        Level::TRACE => ":mag:",
        Level::DEBUG => ":bug:",
        Level::INFO => ":information_source:",
        Level::WARN => ":warning:",
        Level::ERROR => ":x:",
    };
    let color = match notification.level() {
        Level::TRACE | Level::DEBUG => 1_752_220,
        Level::INFO => 5_763_719,
        Level::WARN => 15_105_570,
        Level::ERROR => 15_548_997,
    };
    let message: String = notification.message().chars().take(MAX_MESSAGE_CHARS).collect();
    let metadata = serde_json::to_string_pretty(notification.metadata())
        .map_err(|_| Error::InvalidPlatformMessage("Discord metadata could not be serialized"))?;
    let mut fields = vec![
        json!({
            "name": "Target Span",
            "value": format!("`{}::{}`", notification.target(), notification.span()),
            "inline": true
        }),
        json!({
            "name": "Source",
            "value": format!("`{}#L{}`", notification.source_file(), notification.source_line()),
            "inline": true
        }),
    ];
    for (index, chunk) in chunks(&metadata, MAX_FIELD_VALUE_CHARS).into_iter().enumerate() {
        fields.push(json!({
            "name": if index == 0 { "Metadata".to_owned() } else { format!("Metadata ({})", index + 1) },
            "value": format!("```json\n{chunk}\n```"),
            "inline": false
        }));
    }
    DiscordMessage::from_value(json!({
        "embeds": [{
            "title": format!("{} - {} {}", notification.app_name(), emoji, notification.level()),
            "description": format!("```rust\n{message}\n```"),
            "fields": fields,
            "footer": { "text": notification.app_name() },
            "color": color
        }]
    }))
}

fn render_text(notification: &PreparedNotification) -> Result<DiscordMessage, Error> {
    let metadata = serde_json::to_string_pretty(notification.metadata())
        .map_err(|_| Error::InvalidPlatformMessage("Discord metadata could not be serialized"))?;
    DiscordMessage::text(format!(
        "**Trace from {}**\n**Event [{}]**: \"{}\"\n**Target**: {}\n**Span**: {}\n**Metadata**:\n```json\n{}\n```\n**Source**: {}#L{}",
        notification.app_name(),
        notification.level(),
        notification.message(),
        notification.target(),
        notification.span(),
        metadata,
        notification.source_file(),
        notification.source_line()
    ))
}

fn apply_notification_policy(
    mut message: DiscordMessage,
    level: Level,
    configured_target: Option<MentionTarget>,
) -> Result<String, Error> {
    let object = message
        .0
        .as_object_mut()
        .ok_or(Error::InvalidPlatformMessage("Discord message must be an object"))?;
    if let Some(content) = object.get("content").and_then(Value::as_str) {
        object.insert("content".into(), Value::String(neutralize_broadcasts(content)));
    }

    let active_target = (level == Level::ERROR).then_some(configured_target).flatten();
    let (token, parse, roles) = match active_target {
        Some(MentionTarget::Everyone) => (Some("@everyone".to_owned()), vec!["everyone"], Vec::new()),
        Some(MentionTarget::Here) => (Some("@here".to_owned()), vec!["everyone"], Vec::new()),
        Some(MentionTarget::Role(role)) => (
            Some(format!("<@&{}>", role.get())),
            Vec::new(),
            vec![role.get().to_string()],
        ),
        None => (None, Vec::new(), Vec::new()),
    };
    if let Some(token) = token {
        let existing = object.get("content").and_then(Value::as_str).unwrap_or_default();
        let content = if existing.is_empty() {
            token
        } else {
            format!("{token}\n{existing}")
        };
        object.insert("content".into(), Value::String(content));
    }
    object.insert(
        "allowed_mentions".into(),
        json!({
            "parse": parse,
            "roles": roles,
            "users": [],
            "replied_user": false
        }),
    );
    validate_message(&Value::Object(object.clone()))?;
    serde_json::to_string(object).map_err(|_| Error::InvalidPlatformMessage("Discord message could not be serialized"))
}

fn validate_message(value: &Value) -> Result<(), Error> {
    let object = value
        .as_object()
        .ok_or(Error::InvalidPlatformMessage("Discord message must be an object"))?;
    if let Some(content) = object.get("content") {
        let content = content
            .as_str()
            .ok_or(Error::InvalidPlatformMessage("Discord content must be a string"))?;
        if content.chars().count() > 2_000 {
            return Err(Error::InvalidPlatformMessage("Discord content exceeds 2000 characters"));
        }
    }
    if let Some(embeds) = object.get("embeds") {
        if !embeds.is_array() {
            return Err(Error::InvalidPlatformMessage("Discord embeds must be an array"));
        }
    }
    if !object.contains_key("content") && !object.contains_key("embeds") {
        return Err(Error::InvalidPlatformMessage(
            "Discord message has no content or embeds",
        ));
    }
    Ok(())
}

fn neutralize_broadcasts(content: &str) -> String {
    content
        .replace("@everyone", "@\u{200b}everyone")
        .replace("@here", "@\u{200b}here")
}

fn chunks(value: &str, size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if current.chars().count() == size {
            chunks.push(current);
            current = String::new();
        }
        current.push(character);
    }
    if !current.is_empty() || chunks.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use regex::Regex;
    use serde_json::Map;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    static ENVIRONMENT: Mutex<()> = Mutex::new(());

    struct SourceLessCallsite;

    static SOURCELESS_CALLSITE: SourceLessCallsite = SourceLessCallsite;
    static SOURCELESS_METADATA: tracing::Metadata<'static> = tracing::Metadata::new(
        "source-less",
        "source-less-target",
        Level::INFO,
        None,
        None,
        None,
        tracing::field::FieldSet::new(&["message"], tracing::callsite::Identifier(&SOURCELESS_CALLSITE)),
        tracing::metadata::Kind::EVENT,
    );

    impl tracing::callsite::Callsite for SourceLessCallsite {
        fn set_interest(&self, _interest: tracing::subscriber::Interest) {}

        fn metadata(&self) -> &tracing::Metadata<'_> {
            &SOURCELESS_METADATA
        }
    }

    #[derive(Clone, Debug)]
    struct Snapshot {
        message: String,
        target: String,
        span: String,
        metadata: Map<String, Value>,
        source_file: String,
        source_line: u32,
        level: Level,
    }

    struct RecordingRenderer(Arc<Mutex<Vec<Snapshot>>>);

    impl DiscordRenderer for RecordingRenderer {
        fn render(&self, notification: &PreparedNotification) -> Result<DiscordMessage, Error> {
            self.0.lock().unwrap().push(Snapshot {
                message: notification.message().to_owned(),
                target: notification.target().to_owned(),
                span: notification.span().to_owned(),
                metadata: notification.metadata().clone(),
                source_file: notification.source_file().to_owned(),
                source_line: notification.source_line(),
                level: notification.level(),
            });
            DiscordMessage::text("custom @everyone @here")
        }
    }

    struct UnsafeRenderer;

    impl DiscordRenderer for UnsafeRenderer {
        fn render(&self, _notification: &PreparedNotification) -> Result<DiscordMessage, Error> {
            DiscordMessage::from_value(json!({
                "content": "custom @everyone @here",
                "allowed_mentions": { "parse": ["everyone", "roles"], "roles": ["999"] }
            }))
        }
    }

    fn apply(target: Option<MentionTarget>, level: Level, content: &str) -> Value {
        let body = apply_notification_policy(DiscordMessage::text(content).unwrap(), level, target).unwrap();
        serde_json::from_str(&body).unwrap()
    }

    async fn webhook() -> (String, oneshot::Receiver<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            let (header_end, content_length) = loop {
                let read = socket.read(&mut chunk).await.unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    let header_end = header_end + 4;
                    let headers = std::str::from_utf8(&request[..header_end]).unwrap();
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_owned)
                        })
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    break (header_end, content_length);
                }
            };
            while request.len() < header_end + content_length {
                let read = socket.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
            }
            let body = serde_json::from_slice(&request[header_end..header_end + content_length]).unwrap();
            sender.send(body).unwrap();
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        (format!("http://{address}/webhook/secret"), receiver)
    }

    #[test]
    fn default_and_non_error_messages_suppress_mentions() {
        for level in [Level::TRACE, Level::DEBUG, Level::INFO, Level::WARN, Level::ERROR] {
            let body = apply(None, level, "trace says @everyone and @here");
            assert_eq!(body["allowed_mentions"]["parse"], json!([]));
            assert_eq!(body["content"], "trace says @\u{200b}everyone and @\u{200b}here");
        }
        let body = apply(Some(MentionTarget::Everyone), Level::WARN, "warning");
        assert_eq!(body["allowed_mentions"]["parse"], json!([]));
        assert_eq!(body["content"], "warning");
    }

    #[test]
    fn error_mentions_have_visible_tokens_and_exact_permissions() {
        let everyone = apply(Some(MentionTarget::Everyone), Level::ERROR, "failure @here");
        assert_eq!(everyone["content"], "@everyone\nfailure @\u{200b}here");
        assert_eq!(everyone["allowed_mentions"]["parse"], json!(["everyone"]));
        assert_eq!(everyone["allowed_mentions"]["roles"], json!([]));

        let here = apply(Some(MentionTarget::Here), Level::ERROR, "failure @everyone");
        assert_eq!(here["content"], "@here\nfailure @\u{200b}everyone");
        assert_eq!(here["allowed_mentions"]["parse"], json!(["everyone"]));

        let role = apply(
            Some(MentionTarget::Role(DiscordRoleId::new(42).unwrap())),
            Level::ERROR,
            "failure",
        );
        assert_eq!(role["content"], "<@&42>\nfailure");
        assert_eq!(role["allowed_mentions"]["parse"], json!([]));
        assert_eq!(role["allowed_mentions"]["roles"], json!(["42"]));
    }

    #[test]
    fn custom_renderer_observes_prepared_notification_through_layer() {
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let (layer, _delivery) = DiscordLayer::builder("app", EventFilters::default(), "https://example.com/hook")
            .unwrap()
            .renderer(RecordingRenderer(recorded.clone()))
            .build();
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(answer = 42, "broken");
        });

        let recorded = recorded.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].message, "broken");
        assert_eq!(recorded[0].level, Level::ERROR);
        assert_eq!(recorded[0].metadata["answer"], json!(42));
        assert!(!recorded[0].source_file.is_empty());
        assert_ne!(recorded[0].source_line, 0);
    }

    #[test]
    fn preparation_preserves_target_message_level_and_field_filters() {
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let target_filters: EventFilters = Regex::new("allowed-target").unwrap().into();
        let message_filters: EventFilters = (Vec::new(), vec![Regex::new("blocked message").unwrap()]).into();
        let event_fields: EventFilters = Regex::new("^keep$").unwrap().into();
        let (layer, _delivery) = DiscordLayer::builder("app", target_filters, "https://example.com/hook")
            .unwrap()
            .message_filters(message_filters)
            .event_by_field_filters(event_fields)
            .field_exclusion_filters(vec![Regex::new("^secret$").unwrap()])
            .level_filter("info")
            .renderer(RecordingRenderer(recorded.clone()))
            .build();
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::event!(target: "blocked-target", Level::INFO, keep = 1, "visible");
            tracing::event!(target: "allowed-target", Level::INFO, keep = 2, "blocked message");
            tracing::event!(target: "allowed-target", Level::DEBUG, keep = 3, "visible");
            tracing::event!(target: "allowed-target", Level::INFO, other = 4, "visible");
            tracing::event!(target: "allowed-target", Level::INFO, keep = 5, secret = 6, "visible");
        });

        let recorded = recorded.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].target, "allowed-target");
        assert_eq!(recorded[0].metadata["keep"], json!(5));
        assert!(!recorded[0].metadata.contains_key("secret"));
    }

    #[test]
    fn preparation_preserves_message_fallbacks() {
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let (layer, _delivery) = DiscordLayer::builder("app", EventFilters::default(), "https://example.com/hook")
            .unwrap()
            .renderer(RecordingRenderer(recorded.clone()))
            .build();
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::event!(Level::ERROR, error = "fallback error");
            tracing::event!(Level::INFO, answer = 42);
        });

        let recorded = recorded.lock().unwrap();
        assert_eq!(recorded[0].message, "fallback error");
        assert_eq!(recorded[1].message, "No message");
    }

    #[test]
    fn preparation_preserves_absent_source_fallbacks() {
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let (layer, _delivery) = DiscordLayer::builder("app", EventFilters::default(), "https://example.com/hook")
            .unwrap()
            .renderer(RecordingRenderer(recorded.clone()))
            .build();
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            let field = SOURCELESS_METADATA.fields().field("message").unwrap();
            let message = "source-less event";
            let values = [(&field, Some(&message as &dyn tracing::field::Value))];
            tracing::Event::dispatch(&SOURCELESS_METADATA, &SOURCELESS_METADATA.fields().value_set(&values));
        });

        let recorded = recorded.lock().unwrap();
        assert_eq!(recorded[0].message, "source-less event");
        assert_eq!(recorded[0].source_file, "Unknown");
        assert_eq!(recorded[0].source_line, 0);
    }

    #[test]
    fn current_span_fields_append_after_event_fields_and_win_overlaps() {
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let (layer, _delivery) = DiscordLayer::builder("app", EventFilters::default(), "https://example.com/hook")
            .unwrap()
            .field_exclusion_filters(vec![Regex::new("span_only").unwrap()])
            .renderer(RecordingRenderer(recorded.clone()))
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_bunyan_formatter::JsonStorageLayer)
            .with(layer);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("current", shared = "span", span_only = 7);
            let _entered = span.enter();
            tracing::info!(shared = "event", event_only = 9, "inside span");
        });

        let recorded = recorded.lock().unwrap();
        assert_eq!(recorded[0].span, "current");
        assert_eq!(recorded[0].metadata["shared"], json!("span"));
        assert_eq!(recorded[0].metadata["span_only"], json!(7));
        assert_eq!(recorded[0].metadata["event_only"], json!(9));
    }

    #[test]
    fn role_and_custom_messages_are_validated() {
        assert!(DiscordRoleId::new(0).is_err());
        assert!("not-a-role".parse::<DiscordRoleId>().is_err());
        assert!(DiscordMessage::from_value(json!([])).is_err());
        assert!(DiscordMessage::from_value(json!({ "content": 42 })).is_err());
    }

    #[test]
    fn missing_environment_configuration_is_an_error() {
        let _guard = ENVIRONMENT.lock().unwrap();
        let previous = std::env::var_os("DISCORD_WEBHOOK_URL");
        std::env::remove_var("DISCORD_WEBHOOK_URL");
        let result = DiscordLayer::from_env("app", EventFilters::default());
        if let Some(previous) = previous {
            std::env::set_var("DISCORD_WEBHOOK_URL", previous);
        }
        assert!(matches!(
            result,
            Err(Error::MissingEnvironmentVariable("DISCORD_WEBHOOK_URL"))
        ));
    }

    #[tokio::test]
    async fn platform_layer_and_delivery_apply_policy_end_to_end() {
        let (webhook_url, request) = webhook().await;
        let (layer, delivery) = DiscordLayer::builder("app", EventFilters::default(), webhook_url)
            .unwrap()
            .presentation(DiscordPresentation::Text)
            .mention_target(MentionTarget::Everyone)
            .build();
        let delivery = delivery.spawn().unwrap();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!("failure from @here");
        });

        let report = delivery.shutdown().await.unwrap();
        let body = request.await.unwrap();

        assert_eq!(report.accepted(), 1);
        assert_eq!(report.delivered(), 1);
        assert!(body["content"].as_str().unwrap().starts_with("@everyone\n"));
        assert!(body["content"].as_str().unwrap().contains("@\u{200b}here"));
        assert_eq!(body["allowed_mentions"]["parse"], json!(["everyone"]));
        assert_eq!(body["allowed_mentions"]["roles"], json!([]));
    }

    #[tokio::test]
    async fn rich_is_default_and_suppresses_mentions_end_to_end() {
        let (webhook_url, request) = webhook().await;
        let (layer, delivery) = DiscordLayer::builder("app", EventFilters::default(), webhook_url)
            .unwrap()
            .build();
        let delivery = delivery.spawn().unwrap();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || tracing::info!("ordinary @everyone"));

        let report = delivery.shutdown().await.unwrap();
        let body = request.await.unwrap();

        assert_eq!(report.delivered(), 1);
        assert!(body["embeds"].is_array());
        assert_eq!(body["allowed_mentions"]["parse"], json!([]));
    }

    #[tokio::test]
    async fn custom_renderer_cannot_override_mention_policy() {
        let (webhook_url, request) = webhook().await;
        let (layer, delivery) = DiscordLayer::builder("app", EventFilters::default(), webhook_url)
            .unwrap()
            .renderer(UnsafeRenderer)
            .mention_target(MentionTarget::Role(DiscordRoleId::new(42).unwrap()))
            .build();
        let delivery = delivery.spawn().unwrap();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || tracing::error!("failure"));

        let report = delivery.shutdown().await.unwrap();
        let body = request.await.unwrap();

        assert_eq!(report.delivered(), 1);
        assert_eq!(body["allowed_mentions"]["parse"], json!([]));
        assert_eq!(body["allowed_mentions"]["roles"], json!(["42"]));
        assert_eq!(body["content"], "<@&42>\ncustom @\u{200b}everyone @\u{200b}here");
    }

    #[tokio::test]
    async fn renderer_errors_do_not_panic_or_enqueue() {
        struct InvalidRenderer;
        impl DiscordRenderer for InvalidRenderer {
            fn render(&self, _notification: &PreparedNotification) -> Result<DiscordMessage, Error> {
                Err(Error::InvalidPlatformMessage("test rejection"))
            }
        }

        let (layer, delivery) = DiscordLayer::builder("app", EventFilters::default(), "https://example.com/hook")
            .unwrap()
            .renderer(InvalidRenderer)
            .build();
        let delivery = delivery.spawn().unwrap();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || tracing::error!("failure"));

        let report = delivery.shutdown().await.unwrap();
        assert_eq!(report.accepted(), 0);
    }
}

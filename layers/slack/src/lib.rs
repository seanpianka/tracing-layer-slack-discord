#![doc = include_str!("../README.md")]

use std::sync::Arc;

use serde_json::{json, Value};
use tracing::{Event, Level, Subscriber};
use tracing_layer_core::private::PlatformSupport;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

pub use tracing::level_filters::LevelFilter;
pub use tracing_layer_core::{
    Delivery, DeliveryFailure, DeliveryFailureReason, DeliveryHandle, DeliveryReport, Error, EventFilters,
    PreparedNotification, WebhookUrl,
};

/// Filters Trace Events, renders Slack messages, and queues them for delivery.
pub struct SlackLayer {
    support: PlatformSupport,
    rendering: SlackRendering,
}

impl SlackLayer {
    pub fn builder(
        app_name: impl Into<String>,
        target_filters: EventFilters,
        webhook_url: impl AsRef<str>,
    ) -> Result<SlackLayerBuilder, Error> {
        Ok(SlackLayerBuilder::new(
            app_name.into(),
            target_filters,
            WebhookUrl::parse(webhook_url)?,
        ))
    }

    pub fn from_env(app_name: impl Into<String>, target_filters: EventFilters) -> Result<SlackLayerBuilder, Error> {
        let webhook_url =
            std::env::var("SLACK_WEBHOOK_URL").map_err(|_| Error::MissingEnvironmentVariable("SLACK_WEBHOOK_URL"))?;
        Self::builder(app_name, target_filters, webhook_url)
    }
}

impl<S> Layer<S> for SlackLayer
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let Some(notification) = self.support.prepare(event, ctx) else {
            return;
        };
        let message = match self.rendering.render(&notification) {
            Ok(message) => message,
            Err(_error) => {
                #[cfg(feature = "log-errors")]
                eprintln!("ERROR: failed to render Slack notification: {_error}");
                return;
            }
        };
        let body = match serde_json::to_string(message.as_value()) {
            Ok(body) => body,
            Err(_) => {
                #[cfg(feature = "log-errors")]
                eprintln!("ERROR: failed to serialize Slack notification");
                return;
            }
        };
        if let Err(_error) = self.support.enqueue(body) {
            #[cfg(feature = "log-errors")]
            eprintln!("ERROR: failed to enqueue Slack notification: {_error}");
        }
    }
}

/// Configures a Slack layer and the delivery queue it will use.
pub struct SlackLayerBuilder {
    app_name: String,
    target_filters: EventFilters,
    message_filters: Option<EventFilters>,
    event_by_field_filters: Option<EventFilters>,
    field_exclusion_filters: Option<Vec<regex::Regex>>,
    level_filter: Option<LevelFilter>,
    webhook_url: WebhookUrl,
    rendering: SlackRendering,
}

impl SlackLayerBuilder {
    fn new(app_name: String, target_filters: EventFilters, webhook_url: WebhookUrl) -> Self {
        Self {
            app_name,
            target_filters,
            message_filters: None,
            event_by_field_filters: None,
            field_exclusion_filters: None,
            level_filter: None,
            webhook_url,
            rendering: SlackRendering::BuiltIn(SlackPresentation::Rich),
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

    pub fn level_filter(mut self, level_filter: LevelFilter) -> Self {
        self.level_filter = Some(level_filter);
        self
    }

    pub fn presentation(mut self, presentation: SlackPresentation) -> Self {
        self.rendering = SlackRendering::BuiltIn(presentation);
        self
    }

    pub fn renderer(mut self, renderer: impl SlackRenderer) -> Self {
        self.rendering = SlackRendering::Custom(Arc::new(renderer));
        self
    }

    pub fn build(self) -> (SlackLayer, Delivery) {
        let (support, delivery) = PlatformSupport::new(
            self.app_name,
            self.target_filters,
            self.message_filters,
            self.event_by_field_filters,
            self.field_exclusion_filters,
            self.level_filter,
            self.webhook_url,
        );
        (
            SlackLayer {
                support,
                rendering: self.rendering,
            },
            delivery,
        )
    }
}

/// Chooses Slack's rich or plain-text renderer at runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlackPresentation {
    Rich,
    Text,
}

/// A Slack message that passed this crate's structural checks and has no destination.
#[derive(Clone, Debug)]
pub struct SlackMessage(Value);

impl SlackMessage {
    pub fn from_value(value: Value) -> Result<Self, Error> {
        validate_message(&value)?;
        Ok(Self(value))
    }

    pub fn text(content: impl Into<String>) -> Result<Self, Error> {
        Self::from_value(json!({ "text": content.into() }))
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

/// Turns a read-only Prepared Notification into a Slack message.
pub trait SlackRenderer: Send + Sync + 'static {
    fn render(&self, notification: &PreparedNotification) -> Result<SlackMessage, Error>;
}

enum SlackRendering {
    BuiltIn(SlackPresentation),
    Custom(Arc<dyn SlackRenderer>),
}

impl SlackRendering {
    fn render(&self, notification: &PreparedNotification) -> Result<SlackMessage, Error> {
        match self {
            Self::BuiltIn(SlackPresentation::Rich) => render_rich(notification),
            Self::BuiltIn(SlackPresentation::Text) => render_text(notification),
            Self::Custom(renderer) => renderer.render(notification),
        }
    }
}

fn render_rich(notification: &PreparedNotification) -> Result<SlackMessage, Error> {
    let emoji = match notification.level() {
        Level::TRACE => ":mag:",
        Level::DEBUG => ":bug:",
        Level::INFO => ":information_source:",
        Level::WARN => ":warning:",
        Level::ERROR => ":x:",
    };
    let metadata = serde_json::to_string_pretty(notification.metadata())
        .map_err(|_| Error::InvalidPlatformMessage("Slack metadata could not be serialized"))?;
    let escaped_message = escape_slack(notification.message());
    let escaped_target = escape_slack(notification.target());
    let escaped_span = escape_slack(notification.span());
    let escaped_source = escape_slack(notification.source_file());
    let escaped_metadata = escape_slack(&metadata);
    SlackMessage::from_value(json!({
        "link_names": false,
        "blocks": [
            {
                "type": "context",
                "elements": [{
                    "type": "mrkdwn",
                    "text": format!("{} - {} *{}*", escape_slack(notification.app_name()), emoji, notification.level())
                }]
            },
            {
                "type": "section",
                "text": { "type": "mrkdwn", "text": format!("\"_{escaped_message}_\"") }
            },
            {
                "type": "section",
                "fields": [
                    { "type": "mrkdwn", "text": format!("*Target Span*\n{escaped_target}::{escaped_span}") },
                    { "type": "mrkdwn", "text": format!("*Source*\n{escaped_source}#L{}", notification.source_line()) }
                ]
            },
            { "type": "section", "text": { "type": "mrkdwn", "text": "*Metadata:*" } },
            {
                "type": "section",
                "text": { "type": "mrkdwn", "text": format!("```\n{escaped_metadata}\n```") }
            }
        ]
    }))
}

fn render_text(notification: &PreparedNotification) -> Result<SlackMessage, Error> {
    let metadata = serde_json::to_string_pretty(notification.metadata())
        .map_err(|_| Error::InvalidPlatformMessage("Slack metadata could not be serialized"))?;
    SlackMessage::from_value(json!({
        "text": format!(
            "*Trace from {}*\n*Event [{}]*: \"{}\"\n*Target*: _{}_\n*Span*: _{}_\n*Metadata*:\n```\n{}\n```\n*Source*: _{}#L{}_",
            escape_slack(notification.app_name()),
            notification.level(),
            escape_slack(notification.message()),
            escape_slack(notification.target()),
            escape_slack(notification.span()),
            escape_slack(&metadata),
            escape_slack(notification.source_file()),
            notification.source_line()
        ),
        "link_names": false,
        "mrkdwn": true
    }))
}

fn validate_message(value: &Value) -> Result<(), Error> {
    let object = value
        .as_object()
        .ok_or(Error::InvalidPlatformMessage("Slack message must be an object"))?;
    let has_text = if let Some(text) = object.get("text") {
        let text = text
            .as_str()
            .ok_or(Error::InvalidPlatformMessage("Slack text must be a string"))?;
        !text.is_empty()
    } else {
        false
    };
    let has_blocks = if let Some(blocks) = object.get("blocks") {
        let blocks = blocks
            .as_array()
            .ok_or(Error::InvalidPlatformMessage("Slack blocks must be an array"))?;
        if blocks.len() > 50 {
            return Err(Error::InvalidPlatformMessage("Slack message exceeds 50 blocks"));
        }
        if blocks.iter().any(|block| !block.is_object()) {
            return Err(Error::InvalidPlatformMessage("Slack blocks must be objects"));
        }
        !blocks.is_empty()
    } else {
        false
    };
    if !has_text && !has_blocks {
        return Err(Error::InvalidPlatformMessage(
            "Slack message has no non-empty text or blocks",
        ));
    }
    Ok(())
}

fn escape_slack(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::Map;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    static ENVIRONMENT: Mutex<()> = Mutex::new(());

    type RecordedNotifications = Arc<Mutex<Vec<(String, Level, Map<String, Value>)>>>;

    struct RecordingRenderer(RecordedNotifications);

    impl SlackRenderer for RecordingRenderer {
        fn render(&self, notification: &PreparedNotification) -> Result<SlackMessage, Error> {
            self.0.lock().unwrap().push((
                notification.message().to_owned(),
                notification.level(),
                notification.metadata().clone(),
            ));
            SlackMessage::text("custom")
        }
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
    fn default_rendering_escapes_trace_controlled_broadcasts() {
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let (layer, _delivery) = SlackLayer::builder("app", EventFilters::default(), "https://example.com/hook")
            .unwrap()
            .renderer(RecordingRenderer(recorded.clone()))
            .build();
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(payload = "<!channel>", "broken <!here>");
        });

        let recorded = recorded.lock().unwrap();
        assert_eq!(recorded[0].0, "broken <!here>");
        assert_eq!(recorded[0].2["payload"], json!("<!channel>"));
        assert_eq!(escape_slack(&recorded[0].0), "broken &lt;!here&gt;");
        assert_eq!(
            escape_slack(recorded[0].2["payload"].as_str().unwrap()),
            "&lt;!channel&gt;"
        );
    }

    #[test]
    fn rich_is_default_and_text_is_runtime_selectable() {
        let (_rich_layer, _rich_delivery) =
            SlackLayer::builder("app", EventFilters::default(), "https://example.com/hook")
                .unwrap()
                .build();
        let (_text_layer, _text_delivery) =
            SlackLayer::builder("app", EventFilters::default(), "https://example.com/hook")
                .unwrap()
                .presentation(SlackPresentation::Text)
                .build();
    }

    #[test]
    fn custom_messages_are_validated() {
        assert!(SlackMessage::from_value(json!([])).is_err());
        assert!(SlackMessage::from_value(json!({ "text": 42 })).is_err());
        assert!(SlackMessage::from_value(json!({ "blocks": "not-an-array" })).is_err());
        assert!(SlackMessage::from_value(json!({ "text": "" })).is_err());
        assert!(SlackMessage::from_value(json!({ "blocks": [] })).is_err());
        assert!(SlackMessage::from_value(json!({ "blocks": [42] })).is_err());
    }

    #[test]
    fn missing_environment_configuration_is_an_error() {
        let _guard = ENVIRONMENT.lock().unwrap();
        let previous = std::env::var_os("SLACK_WEBHOOK_URL");
        std::env::remove_var("SLACK_WEBHOOK_URL");
        let result = SlackLayer::from_env("app", EventFilters::default());
        if let Some(previous) = previous {
            std::env::set_var("SLACK_WEBHOOK_URL", previous);
        }
        assert!(matches!(
            result,
            Err(Error::MissingEnvironmentVariable("SLACK_WEBHOOK_URL"))
        ));
    }

    #[tokio::test]
    async fn platform_layer_and_delivery_escape_broadcasts_end_to_end() {
        let (webhook_url, request) = webhook().await;
        let (layer, delivery) = SlackLayer::builder("app", EventFilters::default(), webhook_url)
            .unwrap()
            .presentation(SlackPresentation::Text)
            .build();
        let delivery = delivery.spawn().unwrap();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(payload = "<!channel>", "failure from <!here>");
        });

        let report = delivery.shutdown().await.unwrap();
        let body = request.await.unwrap();

        assert_eq!(report.accepted(), 1);
        assert_eq!(report.delivered(), 1);
        assert!(body["text"].as_str().unwrap().contains("&lt;!here&gt;"));
        assert!(body["text"].as_str().unwrap().contains("&lt;!channel&gt;"));
        assert_eq!(body["link_names"], false);
    }

    #[tokio::test]
    async fn rich_is_default_and_escapes_broadcasts_end_to_end() {
        let (webhook_url, request) = webhook().await;
        let (layer, delivery) = SlackLayer::builder("app", EventFilters::default(), webhook_url)
            .unwrap()
            .build();
        let delivery = delivery.spawn().unwrap();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(payload = "<!channel>", "failure from <!here>");
        });

        let report = delivery.shutdown().await.unwrap();
        let body = request.await.unwrap();

        assert_eq!(report.delivered(), 1);
        assert!(body["blocks"].is_array());
        let serialized = serde_json::to_string(&body["blocks"]).unwrap();
        assert!(serialized.contains("&lt;!here&gt;"));
        assert!(serialized.contains("&lt;!channel&gt;"));
        assert_eq!(body["link_names"], false);
    }

    #[tokio::test]
    async fn renderer_errors_do_not_panic_or_enqueue() {
        struct InvalidRenderer;
        impl SlackRenderer for InvalidRenderer {
            fn render(&self, _notification: &PreparedNotification) -> Result<SlackMessage, Error> {
                Err(Error::InvalidPlatformMessage("test rejection"))
            }
        }

        let (layer, delivery) = SlackLayer::builder("app", EventFilters::default(), "https://example.com/hook")
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

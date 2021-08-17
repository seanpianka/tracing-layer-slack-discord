use serde::Serialize;

/// The message sent to Slack. The logged record being "drained" will be
/// converted into this format.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SlackPayload {
    channel: String,
    username: String,
    text: String,
    #[serde(skip_serializing)]
    webhook_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_emoji: Option<String>,
}

impl SlackPayload {
    pub(crate) fn new(
        channel: String,
        username: String,
        text: String,
        webhook_url: String,
        icon_emoji: Option<String>,
    ) -> Self {
        Self {
            channel,
            username,
            text,
            webhook_url,
            icon_emoji,
        }
    }
}

impl SlackPayload {
    pub fn webhook_url(&self) -> &str {
        self.webhook_url.as_str()
    }
}

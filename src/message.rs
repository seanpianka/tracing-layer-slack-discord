use serde::Serialize;

/// The message sent to Slack. The logged record being "drained" will be
/// converted into this format.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SlackPayload {
    text: String,
    #[serde(skip_serializing)]
    webhook_url: String,
}

impl SlackPayload {
    pub(crate) fn new(text: String, webhook_url: String) -> Self {
        Self { text, webhook_url }
    }
}

impl SlackPayload {
    pub fn webhook_url(&self) -> &str {
        self.webhook_url.as_str()
    }
}

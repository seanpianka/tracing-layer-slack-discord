use serde::Serialize;

/// The message sent to Slack. The logged record being "drained" will be
/// converted into this format.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SlackPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<String>,
    #[serde(skip_serializing)]
    webhook_url: String,
}

#[allow(dead_code)]
pub(crate) enum PayloadMessageType {
    Text(String),
    Blocks(String),
}

impl SlackPayload {
    pub(crate) fn new(payload: PayloadMessageType, webhook_url: String) -> Self {
        let text;
        let blocks;
        match payload {
            PayloadMessageType::Text(t) => {
                text = Some(t);
                blocks = None;
            }
            PayloadMessageType::Blocks(b) => {
                text = None;
                blocks = Some(b);
            }
        }
        Self {
            text,
            blocks,
            webhook_url,
        }
    }
}

impl SlackPayload {
    pub fn webhook_url(&self) -> &str {
        self.webhook_url.as_str()
    }
}

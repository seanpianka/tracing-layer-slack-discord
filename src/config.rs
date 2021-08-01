/// Configuration describing how to forward tracing events to Slack.
pub struct SlackConfig {
    pub(crate) channel_name: String,
    pub(crate) username: String,
    pub(crate) icon_emoji: Option<String>,
    pub(crate) webhook_url: String,
}


impl SlackConfig {
    const DEFAULT_EMOJI: &'static str = "robot";

    pub fn new(webhook_url: String, channel_name: String, username: String, icon_emoji: Option<String>) -> Self {
        Self {
            channel_name,
            username,
            icon_emoji,
            webhook_url,
        }
    }
}

impl Default for SlackConfig {
    fn default() -> Self {
        Self::new(
            std::env::var("SLACK_WEBHOOK_URL").expect("slack webhook url in env"),
            std::env::var("SLACK_CHANNEL_NAME").expect("slack channel name in env"),
            std::env::var("SLACK_USERNAME").expect("slack username in env"),
            std::env::var("SLACK_EMOJI").ok().or_else(|| Some(String::from(Self::DEFAULT_EMOJI))),
        )
    }
}

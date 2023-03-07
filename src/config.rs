/// Configuration describing how to forward tracing events to Slack.
pub struct SlackConfig {
    pub(crate) webhook_url: String,
}

impl SlackConfig {
    pub fn new(webhook_url: String) -> Self {
        Self { webhook_url }
    }

    /// Create a new config for forwarding messages to Slack using configuration
    /// available in the environment.
    ///
    /// Required env vars:
    ///   * SLACK_WEBHOOK_URL
    pub fn new_from_env() -> Self {
        Self::new(std::env::var("SLACK_WEBHOOK_URL").expect("slack webhook url in env"))
    }
}

impl Default for SlackConfig {
    fn default() -> Self {
        Self::new_from_env()
    }
}

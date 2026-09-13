use std::fmt;

/// An error produced while configuring or operating a trace notification layer.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    MissingEnvironmentVariable(&'static str),
    InvalidWebhookUrl,
    InvalidConfiguration(&'static str),
    InvalidPlatformMessage(&'static str),
    RuntimeUnavailable,
    DeliveryStopped,
    WorkerJoin,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnvironmentVariable(name) => write!(formatter, "missing environment variable {name}"),
            Self::InvalidWebhookUrl => formatter.write_str("invalid webhook URL"),
            Self::InvalidConfiguration(reason) => write!(formatter, "invalid configuration: {reason}"),
            Self::InvalidPlatformMessage(reason) => write!(formatter, "invalid platform message: {reason}"),
            Self::RuntimeUnavailable => formatter.write_str("delivery requires an active Tokio runtime"),
            Self::DeliveryStopped => formatter.write_str("webhook delivery is no longer accepting messages"),
            Self::WorkerJoin => formatter.write_str("webhook delivery worker failed"),
        }
    }
}

impl std::error::Error for Error {}

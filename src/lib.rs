pub use config::SlackConfig;
pub use layer::SlackLayer;
pub use layer::SlackLayerBuilder;
pub use worker::WorkerMessage;
pub use matches::EventFilters;

mod config;
mod layer;
mod message;
mod worker;
mod matches;

pub type ChannelSender = tokio::sync::mpsc::UnboundedSender<WorkerMessage>;
pub(crate) type ChannelReceiver = tokio::sync::mpsc::UnboundedReceiver<WorkerMessage>;

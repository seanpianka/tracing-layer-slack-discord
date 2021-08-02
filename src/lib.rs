pub use config::SlackConfig;
pub use layer::SlackForwardingLayer;
pub use worker::WorkerMessage;
pub use matches::EventFilters;

mod config;
mod layer;
mod message;
mod worker;
mod matches;

pub type ChannelSender = tokio::sync::mpsc::UnboundedSender<WorkerMessage>;
pub(crate) type ChannelReceiver = tokio::sync::mpsc::UnboundedReceiver<WorkerMessage>;

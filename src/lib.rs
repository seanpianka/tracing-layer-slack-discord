#![doc = include_str!("../README.md")]
pub use config::SlackConfig;
pub use layer::SlackLayer;
pub use layer::SlackLayerBuilder;
pub use worker::SlackBackgroundWorker;
pub use matches::EventFilters;

use crate::worker::WorkerMessage;

mod config;
mod layer;
mod matches;
mod message;
mod worker;

pub(crate) type ChannelSender = tokio::sync::mpsc::UnboundedSender<WorkerMessage>;
pub(crate) type ChannelReceiver = tokio::sync::mpsc::UnboundedReceiver<WorkerMessage>;

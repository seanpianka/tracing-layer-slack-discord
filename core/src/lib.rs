//! Shared trace preparation and webhook delivery for the platform crates.

mod error;
pub mod filters;
mod layer;
mod worker;

pub use error::Error;
pub use filters::EventFilters;
pub use layer::PreparedNotification;
pub use worker::{Delivery, DeliveryFailure, DeliveryFailureReason, DeliveryHandle, DeliveryReport, WebhookUrl};

#[doc(hidden)]
pub mod private {
    pub use crate::layer::{prepare_event, PreparationConfig};
    pub use crate::worker::{delivery_channel, Enqueue};
}

//! Filters Trace Events into platform-neutral notifications and delivers rendered messages.

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
    use regex::Regex;
    use tracing::level_filters::LevelFilter;
    use tracing::{Event, Subscriber};
    use tracing_subscriber::layer::Context;

    use crate::layer::{prepare_event, PreparationConfig};
    use crate::worker::{delivery_channel, Enqueue};
    use crate::{Delivery, Error, EventFilters, PreparedNotification, WebhookUrl};

    /// Keeps event preparation and delivery wiring out of the platform crates.
    pub struct PlatformSupport {
        app_name: String,
        preparation: PreparationConfig,
        enqueue: Enqueue,
    }

    impl PlatformSupport {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            app_name: String,
            target_filters: EventFilters,
            message_filters: Option<EventFilters>,
            event_by_field_filters: Option<EventFilters>,
            field_exclusion_filters: Option<Vec<Regex>>,
            level_filter: Option<LevelFilter>,
            webhook_url: WebhookUrl,
        ) -> (Self, Delivery) {
            let (enqueue, delivery) = delivery_channel(webhook_url);
            (
                Self {
                    app_name,
                    preparation: PreparationConfig::new(
                        target_filters,
                        message_filters,
                        event_by_field_filters,
                        field_exclusion_filters,
                        level_filter,
                    ),
                    enqueue,
                },
                delivery,
            )
        }

        pub fn prepare<S>(&self, event: &Event<'_>, ctx: Context<'_, S>) -> Option<PreparedNotification>
        where
            S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
        {
            prepare_event(&self.app_name, &self.preparation, event, ctx)
        }

        pub fn enqueue(&self, body: String) -> Result<(), Error> {
            self.enqueue.send(body)
        }
    }
}

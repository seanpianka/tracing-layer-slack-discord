# tracing-layer-slack

`tracing-layer-slack` provides a [`Layer`] implementation based on top of a [`tracing`] [`Subscriber`] and [`tracing-bunyan-formatter`]'s [`JsonStorageLayer`]:
- [`JsonStorageLayer`], to attach contextual information to spans for ease of consumption by
  downstream [`Layer`]s, via [`JsonStorage`] and [`Span`]'s [`extensions`](https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/registry/struct.ExtensionsMut.html);
- [`SlackForwardingLayer`], which sends an HTTP POST request (via [`tokio`] and [`reqwest`]) to a user-defined Slack webhook URL upon event creation. 

## Installation

Configure the dependencies and pull directly from GitHub:

```toml
[dependencies]
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
tracing-futures = "0.2"
tracing-bunyan-formatter = { version = "0.2", default-features = false }
tracing-layer-slack = { git = "https://github.com/seanpianka/tracing-layer-slack", branch = "master" }
```

## Examples 

### Simple

```rust
use std::time::Duration;

use regex::Regex;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, SlackLayer};

#[instrument]
pub async fn create_user(id: u64) {
    for i in 0..2 {
        network_io(i).await;
    }
    info!(param = id, "A user was created");
}

#[instrument]
pub async fn network_io(id: u64) {
    info!(id, "We did our network I/O thing");
}

pub async fn controller() {
    info!("Orphan event without a parent span");
    create_user(2).await;
    tokio::time::sleep(Duration::from_secs(5)).await;
    create_user(4).await;
    tokio::time::sleep(Duration::from_secs(5)).await;
    create_user(6).await;
}

#[tokio::main]
async fn main() {
    // Only show events from where this example code is the target.
    let target_to_filter: EventFilters = Regex::new("simple").unwrap().into();

    // Initialize the layer and an async background task for sending our Slack messages.
    let (slack_layer, mut background_worker) = SlackLayer::builder(target_to_filter).build();
    // Initialize the global default subscriber for tracing events.
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // Spawn a worker future on the tokio runtime to send our Slack messages.
    background_worker.startup();
    // Perform our application code that needs tracing and Slack messages.
    controller().await;
    // Waits for all Slack messages to be sent before exiting.
    background_worker.teardown().await;
}
```

[`Layer`]: https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/layer/trait.Layer.html
[`SlackForwardingLayer`]: https://docs.rs/tracing-layer-slack/0.1.0/tracing_layer_slack/struct.SlackForwardingLayer.html
[`JsonStorageLayer`]: https://docs.rs/tracing-bunyan-formatter/0.1.6/tracing_bunyan_formatter/struct.JsonStorageLayer.html
[`JsonStorage`]: https://docs.rs/tracing-bunyan-formatter/0.1.6/tracing_bunyan_formatter/struct.JsonStorage.html
[`tracing-bunyan-formatter`]: https://docs.rs/tracing-bunyan-formatter/0.2.4/tracing_bunyan_formatter/index.html
[`Span`]: https://docs.rs/tracing/0.1.13/tracing/struct.Span.html
[`Subscriber`]: https://docs.rs/tracing-core/0.1.10/tracing_core/subscriber/trait.Subscriber.html
[`tracing`]: https://docs.rs/tracing
[`tracing`]: https://docs.rs/tracing-subscriber
[`reqwest`]: https://docs.rs/reqwest/0.11.4/reqwest/
[`tokio`]: https://docs.rs/tokio/1.8.1/tokio/

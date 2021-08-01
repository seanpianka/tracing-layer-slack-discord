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

## Getting Started

```rust
use tracing::info;
use tracing::instrument;
use tracing_bunyan_formatter::JsonStorageLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;
use tracing_layer_slack::{SlackForwardingLayer, SlackConfig, WorkerMessage};

#[instrument]
pub async fn a_unit_of_work(_first_parameter: u64) {
  for i in 0..2 {
    a_sub_unit_of_work(i);
  }
  info!(excited = "true", "Tracing is quite cool!");
}

#[instrument]
pub fn a_sub_unit_of_work(_sub_parameter: u64) {
  info!("Events have the full context of their parent span!");
}

pub async fn handler() {
  info!("Orphan event without a parent span");
  a_unit_of_work(2).await;
}

#[tokio::main]
async fn main() {
  let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
  let worker_handle = tokio::spawn(tracing_layer_slack::worker(rx));
  let slack_layer = SlackForwardingLayer::new("simple".into(), SlackConfig::default(), tx.clone());
  let subscriber = Registry::default().with(JsonStorageLayer).with(slack_layer);
  tracing::subscriber::set_global_default(subscriber).unwrap();
  handler().await;
  tx.send(WorkerMessage::Shutdown);
  worker_handle.await;
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

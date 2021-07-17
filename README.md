# tracing-layer-slack

`tracing-layer-slack` provides a [`Layer`] implementation based on top of a [`tracing`] [`Subscriber`] and [`tracing-bunyan-formatter`]'s [`JsonStorageLayer`]:
- [`JsonStorageLayer`], to attach contextual information to spans for ease of consumption by
  downstream [`Layer`]s, via [`JsonStorage`] and [`Span`]'s [`extensions`](https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/registry/struct.ExtensionsMut.html);
- [`SlackForwardingLayer`]`, which sends an HTTP POST request (via [`reqwest`]) to a user-defined Slack webhook URL upon event creation. 

## Installation
For the bleeding edge, pull directly from master:

```toml
[dependencies]
tracing = "0.1"
tracing-futures = "0.2"
tracing-bunyan-formatter = { version = "0.2", default-features = false }
tracing-layer-slack = { git = "https://github.com/seanpianka/tracing-layer-slack", branch = "master" }
```

## Getting Started

```rust
use tracing_slack_layer::SlackForwardingLayer;
use tracing_bunyan_formatter::JsonStorageLayer;
use tracing::instrument;
use tracing::info;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

#[instrument]
pub fn a_unit_of_work(first_parameter: u64) {
    for i in 0..2 {
        a_sub_unit_of_work(i);
    }
    info!(excited = "true", "Tracing is quite cool!");
}

#[instrument]
pub fn a_sub_unit_of_work(sub_parameter: u64) {
    info!("Events have the full context of their parent span!");
}

fn main() {
    let slack_layer = SlackForwardingLayer::new(
        "https://slack.com/webhook_url".into(), 
        "project_traces".into(), 
        "Tracing Bot".into(), 
        Some(":robot:".into()),
    );
    let subscriber = Registry::default()
        .with(JsonStorageLayer)
        .with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    info!("Orphan event without a parent span");
    a_unit_of_work(2);
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

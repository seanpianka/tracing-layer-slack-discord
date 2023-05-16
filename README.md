# tracing-layer-slack
[![Docs](https://docs.rs/tracing-layer-slack/badge.svg)](https://docs.rs/tracing-layer-slack)
[![Crates.io](https://img.shields.io/crates/v/tracing-layer-slack.svg?maxAge=2592000)](https://crates.io/crates/tracing-layer-slack)

`tracing-layer-slack` provides a [`Layer`] implementation for sending [`tracing`] events to Slack. 

## Synopsis

[`SlackLayer`] sends POST requests via [`tokio`] and [`reqwest`] to a [Slack Webhook URL](https://api.slack.com/messaging/webhooks) for each new tracing event. The format of the `text` field is statically defined.

This layer also looks for an optional [`JsonStorageLayer`] [`extension`](https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/registry/struct.ExtensionsMut.html) on the parent [`span`] of each event. This extension may contain additional contextual information for the parent span of an event, which is included into the Slack message. 

## Installation

Configure the dependencies and pull directly from GitHub:

```toml
[dependencies]
tokio = "1.0"
tracing = "0.1"
tracing-layer-slack = "0.6"
```

## Examples 

See the full list of examples in [examples/](./examples).

### Simple

In this simple example, a layer is created using Slack configuration in the environment. An orphaned event (one with no parent span) and an event occurring within a span are created in three separate futures, and a number of messages are sent quickly to Slack.

#### Slack Messages

This screenshots shows the first three Slack messages sent while running this example. More messages are sent but were truncated from these images.

##### Slack Blocks

By default, messages are sent using [Slack Blocks](https://api.slack.com/block-kit). Here's an example:

<img src="https://i.imgur.com/76V50Gr.png" title="hover text" alt="Screenshot demonstrating the current formatter implementation for events sent as Slack messages">

##### Slack Text

By disabling the default features of this crate (and therefore disabling the `blocks` feature), you can revert to the older format which does not use the block kit.

<img src="https://i.imgur.com/vefquEK.png" width="450" title="hover text" alt="Screenshot demonstrating the current formatter implementation for events sent as Slack messages">

#### Code example

Run this example locally using the following commands:
```shell
$ git clone https://github.com/seanpianka/tracing-layer-slack.git
$ cd tracing-layer-slack
$ cargo run --example simple
```

You must have Slack configuration exported in the environment.

##### Source
```rust
use regex::Regex;
use tracing::{info, warn, instrument};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, SlackLayer};

#[instrument]
pub async fn create_user(id: u64) -> u64 {
    network_io(id).await;
    info!(param = id, "A user was created");
    id
}

#[instrument]
pub async fn network_io(id: u64) {
    warn!(user_id = id, "some network io happened");
}

pub async fn controller() {
    info!("Orphan event without a parent span");
    let (id1, id2, id3) = tokio::join!(create_user(2), create_user(4), create_user(6));
}

#[tokio::main]
async fn main() {
    // Only show events from where this example code is the target.
    let target_to_filter: EventFilters = Regex::new("simple").unwrap().into();

    // Initialize the layer and an async background task for sending our Slack messages.
    let (slack_layer, background_worker) = SlackLayer::builder("my-app-name".to_string(), target_to_filter).build();
    // Initialize the global default subscriber for tracing events.
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // Perform our application code that needs tracing and Slack messages.
    controller().await;
    // Waits for all Slack messages to be sent before exiting.
    background_worker.shutdown().await;
}
```

[`Layer`]: https://docs.rs/tracing-subscriber/0.3.0/tracing_subscriber/layer/trait.Layer.html
[`SlackLayer`]: https://docs.rs/tracing-layer-slack/0.2.2/tracing_layer_slack/struct.SlackLayer.html
[`Span`]: https://docs.rs/tracing/0.1.13/tracing/struct.Span.html
[`Subscriber`]: https://docs.rs/tracing-core/0.1.10/tracing_core/subscriber/trait.Subscriber.html
[`tracing`]: https://docs.rs/tracing
[`tracing`]: https://docs.rs/tracing-subscriber
[`reqwest`]: https://docs.rs/reqwest/0.11.4/reqwest/
[`tokio`]: https://docs.rs/tokio/1.8.1/tokio/
[`JsonStorageLayer`]: https://docs.rs/tracing-bunyan-formatter/0.3.0/tracing_bunyan_formatter/struct.JsonStorageLayer.html

# tracing-layer-slack-discord

This repository contains [`Layer`] implementations for sending [`tracing`] events to Slack and Discord.

[![tracing-layer-slack](https://img.shields.io/badge/tracing--layer--slack-blue)](https://github.com/seanpianka/tracing-layer-slack/tree/main/layers/slack)
[![tracing-layer-slack on crates.io](https://img.shields.io/crates/v/tracing-layer-slack.svg)](https://crates.io/crates/tracing-layer-slack)
[![Docs](https://docs.rs/tracing-layer-discord/badge.svg)](https://docs.rs/tracing-layer-discord)

[![tracing-layer-discord](https://img.shields.io/badge/tracing--layer--discord-blue)](https://github.com/seanpianka/tracing-layer-slack/tree/main/layers/discord)
[![tracing-layer-discord on crates.io](https://img.shields.io/crates/v/tracing-layer-discord.svg)](https://crates.io/crates/tracing-layer-discord)
[![Docs](https://docs.rs/tracing-layer-slack/badge.svg)](https://docs.rs/tracing-layer-slack)

## Synopsis

[`DiscordLayer`] and [`SlackLayer`] send POST requests via [`tokio`] and [`reqwest`] to a [Discord Webhook URL](https://api.discord.com/messaging/webhooks) and [Slack Webhook URL](https://api.slack.com/messaging/webhooks) for each new tracing event, depending on the user-supplied event filtering rules. The format of the embedded message is statically defined.

This layer also looks for an optional [`JsonStorageLayer`] [`extension`](https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/registry/struct.ExtensionsMut.html) on the parent [`span`] of each event. This extension may contain additional contextual information for the parent span of an event, which is included into the Discord message.

## Features

- Send trace logs to Slack and Discord channels.
- Configurable to suit your needs.
- Easy to integrate with existing Rust applications.

## Usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
tracing-layer-slack = "0"
tracing-layer-discord = "0"
```

Then, in your application:

```rust
use regex::Regex;
use tracing::{info, instrument, warn};
use tracing_subscriber::{layer::SubscriberExt, Registry};

use tracing_layer_slack::{EventFilters, SlackLayer};
use tracing_layer_discord::{EventFilters, DiscordLayer};

#[instrument]
pub async fn network_io(id: u64) {
    warn!(user_id = id, "had to retry the request once");
}

#[tokio::main]
async fn main() {
    // Only show events from where this example code is the target.
    let target_to_filter: EventFilters = Regex::new("simple").unwrap().into();

    let app_name = "test-app".to_string();
    let (slack_layer, slack_worker) = SlackLayer::builder(app_name.clone(), target_to_filter.clone()).build();
    let (discord_layer, discord_worker) = DiscordLayer::builder(app_name, target_to_filter).build();
    let subscriber = Registry::default().with(slack_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // Start the workers and spawn the background async tasks on the current executor.
    discord_worker.start();
    slack_worker.start();

    network_io(123).await;
    
    // Shutdown the workers and ensure their message cache is flushed.
    slack_worker.shutdown().await;
    discord_worker.shutdown().await;
}
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

[`Layer`]: https://docs.rs/tracing-subscriber/0.3.0/tracing_subscriber/layer/trait.Layer.html
[`SlackLayer`]: https://docs.rs/tracing-layer-slack/0.2.2/tracing_layer_slack/struct.SlackLayer.html
[`DiscordLayer`]: https://docs.rs/tracing-layer-discord/0.2.2/tracing_layer_discord/struct.DiscordLayer.html
[`Span`]: https://docs.rs/tracing/0.1.13/tracing/struct.Span.html
[`Subscriber`]: https://docs.rs/tracing-core/0.1.10/tracing_core/subscriber/trait.Subscriber.html
[`tracing`]: https://docs.rs/tracing
[`tracing`]: https://docs.rs/tracing-subscriber
[`reqwest`]: https://docs.rs/reqwest/0.11.4/reqwest/
[`tokio`]: https://docs.rs/tokio/1.8.1/tokio/
[`JsonStorageLayer`]: https://docs.rs/tracing-bunyan-formatter/0.3.0/tracing_bunyan_formatter/struct.JsonStorageLayer.html

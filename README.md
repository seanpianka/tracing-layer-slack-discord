# tracing-layer-slack-discord

Tracing subscriber layers that prepare filtered events and deliver them to Slack or Discord webhooks without performing network I/O in `Layer::on_event`.

## Packages

- [![tracing-layer-slack](https://img.shields.io/badge/tracing--layer--slack-blue)](layers/slack)
  - [![tracing-layer-slack on crates.io](https://img.shields.io/crates/v/tracing-layer-slack.svg)](https://crates.io/crates/tracing-layer-slack) [![tracing-layer-slack documentation](https://docs.rs/tracing-layer-slack/badge.svg)](https://docs.rs/tracing-layer-slack) [![tracing-layer-slack downloads](https://img.shields.io/crates/d/tracing-layer-slack)](https://crates.io/crates/tracing-layer-slack)
  - Slack Block Kit, text messages, custom renderers, and escaped trace-controlled mentions.
- [![tracing-layer-discord](https://img.shields.io/badge/tracing--layer--discord-blue)](layers/discord)
  - [![tracing-layer-discord on crates.io](https://img.shields.io/crates/v/tracing-layer-discord.svg)](https://crates.io/crates/tracing-layer-discord) [![tracing-layer-discord documentation](https://docs.rs/tracing-layer-discord/badge.svg)](https://docs.rs/tracing-layer-discord) [![tracing-layer-discord downloads](https://img.shields.io/crates/d/tracing-layer-discord)](https://crates.io/crates/tracing-layer-discord)
  - Discord embeds, text messages, custom renderers, and opt-in error mentions.
- [![tracing-layer-core](https://img.shields.io/badge/tracing--layer--core-blue)](core)
  - [![tracing-layer-core on crates.io](https://img.shields.io/crates/v/tracing-layer-core.svg)](https://crates.io/crates/tracing-layer-core) [![tracing-layer-core documentation](https://docs.rs/tracing-layer-core/badge.svg)](https://docs.rs/tracing-layer-core) [![tracing-layer-core downloads](https://img.shields.io/crates/d/tracing-layer-core)](https://crates.io/crates/tracing-layer-core)
  - Shared `PreparedNotification` and `WebhookDelivery` behavior.

## tracing-layer-discord

```rust,no_run
use tracing_layer_discord::{DiscordLayer, MentionTarget};
use tracing_subscriber::{layer::SubscriberExt, Registry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (layer, delivery) = DiscordLayer::from_env("api", Default::default())?
        .mention_target(MentionTarget::Everyone)
        .build();
    let delivery = delivery.spawn()?;
    let subscriber = Registry::default().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::error!(request_id = 42, "request failed");
    });

    let report = delivery.shutdown().await?;
    assert_eq!(report.accepted(), report.delivered() + report.failures().len());
    Ok(())
}
```

Discord mentions are disabled by default. A configured `MentionTarget` applies only to `ERROR` events and produces both the visible mention token and a matching `allowed_mentions` restriction. Discord permissions and role mentionability still determine whether members are notified.

## tracing-layer-slack

```rust,no_run
use tracing_layer_slack::{SlackLayer, SlackPresentation};
use tracing_subscriber::{layer::SubscriberExt, Registry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (layer, delivery) = SlackLayer::from_env("api", Default::default())?
        .presentation(SlackPresentation::Text)
        .build();
    let delivery = delivery.spawn()?;
    let subscriber = Registry::default().with(layer);

    tracing::subscriber::with_default(subscriber, || tracing::warn!("request retried"));
    let _report = delivery.shutdown().await?;
    Ok(())
}
```

Rich presentation is the runtime default for both platforms. `from_env` reads `DISCORD_WEBHOOK_URL` or `SLACK_WEBHOOK_URL`; `builder` accepts an explicit URL. Both validate the destination before delivery starts and redact it from diagnostics.

## tracing-layer-core

`tracing-layer-core` owns the platform-neutral notification and delivery flow. Most applications should depend on the Slack or Discord crate instead.

## Delivery guarantees

Webhook Delivery uses an unbounded FIFO queue. It is best-effort and at-least-once: transient or ambiguous failures may result in duplicate notifications. A message is attempted at most five times. Network errors, HTTP 408, HTTP 429, and 5xx responses are retried; other non-2xx responses are terminal. Rate-limit delays are honored within a two-minute per-message budget.

See [the breaking migration guide](docs/migration-0.4.md) when upgrading from the legacy worker and message-factory APIs.

## License

Apache-2.0. See [LICENSE](LICENSE).

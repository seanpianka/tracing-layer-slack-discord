# tracing-layer-slack-discord

Tracing subscriber layers that prepare filtered events and deliver them to Slack or Discord webhooks without performing network I/O in `Layer::on_event`.

## Packages

- [`tracing-layer-discord`](layers/discord): Discord embeds, text messages, custom renderers, and opt-in error mentions.
- [`tracing-layer-slack`](layers/slack): Slack Block Kit, text messages, custom renderers, and escaped trace-controlled mentions.
- [`tracing-layer-core`](core): shared Prepared Notification and Webhook Delivery behavior.

## Discord

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

## Slack

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

## Delivery guarantees

Webhook Delivery uses an unbounded FIFO queue. It is best-effort and at-least-once: transient or ambiguous failures may result in duplicate notifications. A message is attempted at most five times. Network errors, HTTP 408, HTTP 429, and 5xx responses are retried; other non-2xx responses are terminal. Rate-limit delays are honored within a two-minute per-message budget.

See [the breaking migration guide](docs/migration-0.4.md) when upgrading from the legacy worker and message-factory APIs.

## License

Apache-2.0. See [LICENSE](LICENSE).

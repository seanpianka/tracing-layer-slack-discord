# tracing-layer-discord

`tracing-layer-discord` converts filtered tracing events into Discord Platform Messages. Rich embeds are the default; text presentation is selected at runtime.

```rust
use tracing_layer_discord::{DiscordLayer, DiscordPresentation, MentionTarget};

# fn build() -> Result<(), tracing_layer_discord::Error> {
let (layer, delivery) = DiscordLayer::from_env("api", Default::default())?
    .presentation(DiscordPresentation::Rich)
    .mention_target(MentionTarget::Here)
    .build();
# Ok(())
# }
```

Building does not require a Tokio runtime. Call `delivery.spawn()` after entering a runtime, then consume the returned handle with `shutdown().await` to drain accepted messages and receive a `DeliveryReport`.

Mentions are disabled by default and on every non-error event. For `ERROR` events, `MentionTarget::Everyone`, `MentionTarget::Here`, or `MentionTarget::Role` adds a visible token and the exact matching Discord `allowed_mentions` policy. Trace-controlled content and custom renderers cannot widen that policy.

Implement `DiscordRenderer` for custom formatting. It receives a borrowed, read-only `PreparedNotification` and returns a validated `DiscordMessage`; mention policy is sealed after rendering.

The default features select rustls, gzip, and immediate error diagnostics. Use `native-tls` instead of `rustls` when required. Presentation is not a Cargo feature.

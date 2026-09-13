# tracing-layer-slack

`tracing-layer-slack` converts filtered tracing events into Slack Platform Messages. Rich Block Kit presentation is the default; text presentation is selected at runtime.

```rust
use tracing_layer_slack::{SlackLayer, SlackPresentation};

# fn build() -> Result<(), tracing_layer_slack::Error> {
let (layer, delivery) = SlackLayer::from_env("api", Default::default())?
    .presentation(SlackPresentation::Rich)
    .build();
# Ok(())
# }
```

Building does not require a Tokio runtime. Call `delivery.spawn()` after entering a runtime, then consume the returned handle with `shutdown().await` to drain accepted messages and receive a `DeliveryReport`.

Built-in rendering escapes trace-controlled Slack control sequences and disables automatic link-name parsing. Implement `SlackRenderer` for custom formatting; it receives a borrowed, read-only `PreparedNotification` and returns a validated `SlackMessage`.

The default features select rustls, gzip, and immediate error diagnostics. Use `native-tls` instead of `rustls` when required. Presentation is not a Cargo feature.

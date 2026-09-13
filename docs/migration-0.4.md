# Migrating to the platform-first API

This coordinated breaking release moves `tracing-layer-core` and `tracing-layer-discord` to 0.4 and `tracing-layer-slack` to 0.9.

## Construct the destination explicitly

Use `DiscordLayer::builder` or `SlackLayer::builder` with a webhook URL. Use `from_env` to read the platform's existing environment variable. Both forms return an error for missing or malformed configuration instead of panicking.

## Start and stop delivery explicitly

`build` returns `(Layer, Delivery)`. Constructing this pair does not require a Tokio runtime. Inside a runtime, consume `Delivery` with `spawn`; consume the resulting `DeliveryHandle` with `shutdown` to stop acceptance, drain queued messages, and receive a `DeliveryReport`.

The queue is unbounded and FIFO. Delivery is best-effort and at-least-once, so retries after ambiguous failures can produce duplicates.

## Select presentation at runtime

Discord embeds and Slack blocks remain the defaults. Choose text with `DiscordPresentation::Text` or `SlackPresentation::Text`. The former `embed` and `blocks` Cargo features no longer exist.

## Move customization to the platform

Implement `DiscordRenderer` or `SlackRenderer` instead of a core message factory. Each renderer receives a borrowed `PreparedNotification` and returns its platform's validated message type. A Platform Message contains no webhook destination.

Discord mention policy runs after custom rendering. Configure `MentionTarget::Everyone`, `MentionTarget::Here`, or a validated `DiscordRoleId`; only error notifications activate the target.

## Removed core interfaces

The generic webhook layer, core configuration trait, message factory, message input bag, webhook message trait, worker-message command, background worker, and channel aliases were removed without a compatibility facade.

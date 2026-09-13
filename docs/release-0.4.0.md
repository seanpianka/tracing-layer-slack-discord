# Release 0.4.0

This source release publishes three crate versions from the same commit:

- `tracing-layer-core` 0.4.0
- `tracing-layer-discord` 0.4.0
- `tracing-layer-slack` 0.9.0

The repository already has a `v0.4.0` tag for the 2021 Slack release. Keep that tag intact. Tag this release with the package-qualified names `tracing-layer-core-v0.4.0`, `tracing-layer-discord-v0.4.0`, and `tracing-layer-slack-v0.9.0`.

## Release notes

Trace preparation and delivery now sit behind platform-first Slack and Discord APIs. Each platform owns its message format and policy instead of receiving the full core configuration.

This is a breaking release:

- Construct layers with `DiscordLayer::builder`, `DiscordLayer::from_env`, `SlackLayer::builder`, or `SlackLayer::from_env`.
- `build` returns the layer and an unstarted `Delivery`. Start it on a Tokio runtime and call `shutdown` to drain accepted messages.
- Select rich or plain-text rendering at runtime. The former `embed` and `blocks` Cargo features no longer exist.
- Implement `DiscordRenderer` or `SlackRenderer` for custom messages. Each renderer receives a borrowed `PreparedNotification`.
- The generic core layer, configuration, message factory, message, worker command, background worker, and channel aliases are gone.

Discord can mention everyone, active members, or one role on error notifications. The adapter applies the matching `allowed_mentions` policy after rendering, so event fields and custom renderers cannot widen it.

Webhook URLs are validated before startup and redacted from diagnostics. Delivery preserves FIFO order, retries transient failures at most five times, honors bounded rate-limit delays, and reports terminal failures without storing destinations or message bodies.

See [the migration guide](migration-0.4.md) for code examples and delivery guarantees.

## Publish checklist

1. Merge the release commit and work from a clean checkout of `main`.
2. Confirm that CI and dependency review passed for the merge commit.
3. Run the workspace verification commands from `.github/workflows/ci.yml`.
4. Run `cargo publish --dry-run -p tracing-layer-core`.
5. Publish `tracing-layer-core`, then wait until version 0.4.0 resolves from crates.io.
6. Run `cargo publish --dry-run -p tracing-layer-discord` and `cargo publish --dry-run -p tracing-layer-slack`.
7. Publish `tracing-layer-discord`, then `tracing-layer-slack`.
8. Tag the merge commit with the three package-qualified tags and push them.
9. Create one GitHub release from `tracing-layer-discord-v0.4.0` titled `Tracing layers: core 0.4.0, Discord 0.4.0, Slack 0.9.0`, using the release notes above.
10. Confirm each crate page, repository link, and docs.rs build.

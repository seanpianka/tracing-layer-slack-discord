# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.4] - 2024-04-04
### Fixed
- do not unwrap during shutdown

## [0.6.3] - 2024-04-01
### Fixed
- always print error information

## [0.6.2] - 2024-04-01
### Fixed
- do not println debugging logs when compiled in release mode
- make worker type cloneable

## [0.6.1] - 2023-07-20
### Fixed
- Handle network failures in message delivery with retry and exponential backoff

## [0.6.0] - 2023-05-16
### Added
- Messages are now formatted with Slack's Block Kit by default. This can be disabled by disabling the default features so that `blocks` is not enabled.
- Reduced the size of each message and added emojis.
- Added feature flags for enabling gzip compression in reqwest and controlling the usage of native-tls versus rustls.

## [0.5.1] - 2021-10-27
### Added
- Filter messages sent to Slack by their level. Offers optional control over messages sent to Slack, independent of the tracing subscriber's current logging level.

## [0.5.0] - 2021-10-27
### Added
- Remove all configuration except webhook URL, as Slack Apps control all configuration centrally now (and custom integrations are deprecated)

### Documentation
- Add example for filtering traces from being sent to Slack by the content of their messages

## [0.4.0] - 2021-10-27
### Added
- Upgrade to tracing-subscriber and tracing-bunyan-formatter 0.3

### Documentation
- Updated docs.rs URLs to dependencies in README.md


## [0.3.1] - 2021-08-17
### Documentation
- Added documentation for all public items


## [0.3.0] - 2021-08-17
### Changed
- Updated the pretty-printed format for tracing event metadata


## [0.2.2] - 2021-08-16
### Changed
- Updated the pretty-printed format for the tracing event's span/target/source.


## [0.2.1] - 2021-08-03
### Documentation
- Fixed a non-working example in the README


## [0.2.0] - 2021-08-02
### Deprecated
- The background task which parses all tracing events and sends messages to Slack is now spawned immediately when creating the layer. Only the `teardown` method is exposed in the public API.

## [0.1.1] - 2021-08-02
### Changed
- Simplified public API


## [0.1.0] - 2021-08-02
### Added
- Initial release of the slack layer


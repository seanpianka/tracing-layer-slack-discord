# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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


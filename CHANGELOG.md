<!-- SPDX-FileCopyrightText: The Ferrocene Developers -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## Added

- Basic `tracing` functionality for debugging and development. `--verbose` and `-v` are now
  generally accepted and enable debug logging. Passing the flag twice (eg. `-vv`) will enable
  trace logging as well. The `--log-level` argument can accept arbitrary
  [tracing directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives)
  for fine grained control over logging where required.

## [1.0.2] - 2024-07-11

### Added

- Retry downloads in case of network issue (#28).

## [1.0.1] - 2024-05-29

### Fixed

- Versioning issue where `--version` was still showing `0.0.0` (#24).

### Changed

- Update dependencies for all crates in the project workspace (#10).

## [1.0.0] - 2024-05-27

### Added

- Initial public release (#22).

## References

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

[Unreleased]: https://github.com/ferrocene/criticalup/compare/v1.0.2...HEAD
[1.0.2]: https://github.com/ferrocene/criticalup/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/ferrocene/criticalup/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/ferrocene/criticalup/compare/v1.0.0...v1.0.0-prerelease.1

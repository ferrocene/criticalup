<!-- SPDX-FileCopyrightText: The Ferrocene Developers -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Changelog

All notable changes to this project will be documented in this file.

## [1.4.0] - 2025-03-04

### Fixed

- The reworked `criticalup run` behavior was not correctly checking that the toolchain specified
  in `criticalup.toml` was installed. This lead to some situations where users could accidentally 
  run a non-Ferrocene tool when they meant to run Ferrocene tools. This behavior has been altered
  and CriticalUp will now present users with an error suggesting they install the toolchain.

### Added

- New subcommand `init` creates a simple project manifest file in the current directory. A flag `--print` can
  be passed to not save the file and only print the contents.

## [1.3.0] - 2025-01-30

### Changed

- `criticalup run` now behaves more similar to `rustup run` and `uv run`, allowing you to run
  `criticalup run $WHATEVER` and have the respective tool see the appropriate CriticalUp-managed tools
  within the `$PATH` (or equivalent). A `--strict` flag was added to make it possible to ensure only
  tools within the installation are run.

### Added

- Added a `criticalup doc` command which opens the documentation of the relevant Ferrocene version.
- Release instructions to README.
- Subscription management docs.

### Fixed

- Running Clean command now ensures that there are no leftover unused binary proxies.

## [1.2.0] - 2024-11-25

### Changed

- Standardized error messages as close to English rules as possible.
- Changed several CriticalTrust APIs to be async.
- Added a `criticalup verify` command that can be used to verify that a locally installed toolchain.
  is not corrupted or tampered with.
- Added `criticalup archive` which creates an archive of the toolchain for cold storage or backup.

### Fixed

- Bug when using `--offline` mode to install with expired revocation info ends in installation failure. To
  support proper `--offline` mode, the expiration date on revocation info hash must be ignored.

## [1.1.0] - 2024-08-28

### Added

- Support for package revocation added, `criticalup install` will verify packages have not been
  revoked (due to, for example, a security event) before installation.
- An `--offline` flag has been added to `criticalup install`, when enabled only the download cache
  will be used where possible, and the cache will not be populated on cache miss.
- Caching of downloaded keys, manifests, and installation tarballs has been added. Newly downloaded
  artifacts will also be stored in the OS-specific cache directory. The cache can be cleaned with
  `criticalup clean` or any relevant OS behaviors.
- `tracing` support was added for structured and multi-level logging. `--verbose` and `-v` are now
  generally accepted and enable debug logging. Passing the flag twice (eg. `-vv`) will enable
  trace logging as well. The `--log-level` argument can accept arbitrary
  [tracing directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives)
  for fine grained control over logging where required.
- Moved `criticalup` to an async runtime (`tokio`), this resulted in resolving some intermittent
  networking issues on low bandwidth or unreliable connections.

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

[Unreleased]: https://github.com/ferrocene/criticalup/compare/v1.4.0...HEAD

[1.4.0]:  https://github.com/ferrocene/criticalup/compare/v1.3.0...1.4.0

[1.3.0]: https://github.com/ferrocene/criticalup/compare/v1.2.0...v1.3.0

[1.2.0]: https://github.com/ferrocene/criticalup/compare/v1.1.0...v1.2.0

[1.1.0]: https://github.com/ferrocene/criticalup/compare/v1.1.0...v1.0.2

[1.0.2]: https://github.com/ferrocene/criticalup/compare/v1.0.1...v1.0.2

[1.0.1]: https://github.com/ferrocene/criticalup/compare/v1.0.0...v1.0.1

[1.0.0]: https://github.com/ferrocene/criticalup/compare/v1.0.0...v1.0.0-prerelease.1

<!-- SPDX-FileCopyrightText: The Ferrocene Developers -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# CriticalUp

Criticalup is a toolchain manager for [Ferrocene][ferrocene], similar to [`rustup`][rustup].

> [!NOTE]
>
> For normal usage and binary installation, please consult the [CriticalUp Documentation][criticalup-docs].

## Installing

Installation instructions for CriticalUp are included
in [each release](https://github.com/ferrocene/criticalup/releases) as well as the [documentation][criticalup-docs].

## Development

CriticalUp only requires a working Rust and C toolchain to build. [Installation instructions][rust-install] for Rust
typically include installing a C toolchain as well.

## Structure

Criticalup uses a [Cargo Virtual Workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html#virtual-workspace)

### Build

#### Debug

To build a debug version of the development-targeting CriticalUp:

```bash
cargo build -p criticalup-dev
```

To build a debug version of the production-targeting CriticalUp:

```bash
cargo build -p criticalup
```

#### Release

To build a release version:

```bash
cargo build -p criticalup --release
```

### Test

To test CriticalUp from workspace root:

```bash
cargo test --timings --locked
```

To test a CriticalUp specific package from workspace root:

```bash
cargo test -p criticalup-cli --timings --locked
```

## Releasing a new version

To cut a release:

- `git pull` on the `main` branch for latest changes.
- Create and checkout a new release branch from `main`, use the naming convention -  `release/vX.Y.Z`.
  Where, `X.Y.Z` is the release version you are trying to release.
- Update the following on the release branch
    - [crates/criticalup/Cargo.toml](./crates/criticalup/Cargo.toml): Change `version` to `X.Y.Z`.
    - [crates/criticalup-cli/Cargo.toml](./crates/criticalup-cli/Cargo.toml): Change `version` to `X.Y.Z`.
    - [crates/criticalup-dev/Cargo.toml](./crates/criticalup-dev/Cargo.toml): Change `version` to `X.Y.Z`.
    - [crates/criticalup-cli/tests/snapshots/cli__root__version_flags.snap](./crates/criticalup-cli/tests/snapshots/cli__root__version_flags.snap):
      Update this test to match the correct version (`X.Y.Z`).
    - [CHANGELOG.md](./CHANGELOG.md): Make `[Unreleased]` the correct version (`[X.Y.Z]`). Add correct links metadata at
      the bottom.
- Run `cargo test` and `cargo clippy --tests --locked -- -Dwarnings` to make sure there no
  failures.
- Commit and push this branch and open a PR against `main`, on GitHub.
- Wait for approval(s) from reviewer(s).
- Once the PR is approved, comment `bors merge` to merge the PR.
- After the PR is merged, checkout `main` branch and update it (`git pull`) with the latest changes.
- Create a tag `git tag 'vX.Y.Z'`.
- Push the tag `git push origin vX.Y.Z`. This should trigger the release build in GitHub Actions and publish the release
  on its own.
- Create a new PR updating the version to `X.Y.(Z+1)-prerelease.1`, eg `1.5.0` would become `1.5.1-prelease.1`.

If the release build fails:

- Revert the changes from `release/vX.Y.Z` and open a PR to be merged to `main`.
- Delete the tag from GitHub.

## Using ferrocene as default toolchain

To use `ferrocene` as the default `rustup` toolchain, it is possible to create a `rust-toolchain.toml` file at the root:

```
> cat rust-toolchain.toml 
[toolchain]
channel = "ferrocene"
components = ["cargo", "rustfmt", "clippy"]
profile = "default"
```

Add the file to `.gitignore`

[criticalup-docs]: https://criticalup.ferrocene.dev/

[rustup]: https://github.com/rust-lang/rustup

[ferrocene]: https://ferrocene.dev/

[rust-install]: https://www.rust-lang.org/tools/install

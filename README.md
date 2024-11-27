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

To test CriticalUp:

```bash
cargo test
```

## Releasing a new version

We use [`cargo-dist`](https://opensource.axo.dev/cargo-dist/book/quickstart/rust.html) to publish releases.

To cut a release:

- `git pull` on the `main` branch for latest changes.
- Create and checkout a new release branch from `main`, use the naming convention -  `release/vX.Y.Z`.
  Where, `X.Y.Z` is the release version you are trying to release.
- Update the following on the release branch
    - [dist-workspace.toml](./dist-workspace.toml): Change `pr-run-mode = "plan"` to `pr-run-mode = "upload"`.
      **Commit this change separately!** (We will need to drop this commit once the PR passes)
    - [crates/criticalup/Cargo.toml](./crates/criticalup/Cargo.toml): Change `version` to `X.Y.Z`.
    - [crates/criticalup-cli/Cargo.toml](./crates/criticalup-cli/Cargo.toml): Change `version` to `X.Y.Z`.
    - [crates/criticalup-dev/Cargo.toml](./crates/criticalup-dev/Cargo.toml): Change `version` to `X.Y.Z`.
    - [crates/criticalup-cli/tests/snapshots/cli__root__version_flags.snap](./crates/criticalup-cli/tests/snapshots/cli__root__version_flags.snap):
      Update this test to match the correct version (`X.Y.Z`).
    - [CHANGELOG.md](./CHANGELOG.md): Make `[Unreleased]` the correct version (`[X.Y.Z]`). Add correct links metadata at
      the bottom.
- Run `cargo test --workspace` and `cargo clippy --workspace --tests --locked -- -Dwarnings` to make sure there no
  failures.
- Commit and push this branch and open a PR against `main`, on GitHub.
- If the full CI test cycle on the PR passes and the reviewer(s) are OK, drop the
  [dist-workspace.toml](./dist-workspace.toml) commit from above and push.
- Wait for approval(s) from reviewer(s).
- Once the PR is approved, comment `bors merge` to merge the PR.
- After the PR is merged, checkout `main` branch and update it (`git pull`) with the latest changes.
- Create a tag `git tag 'vX.Y.Z'`.
- Push the tag `git push origin vX.Y.Z`. This should trigger the release build in GitHub Actions and publish the release
  on its own.

If the release build fails:

- Revert the changes from `release/vX.Y.Z` and open a PR to be merged to `main`.
- Delete the tag from GitHub.

[criticalup-docs]: https://criticalup.ferrocene.dev/

[rustup]: https://github.com/rust-lang/rustup

[ferrocene]: https://ferrocene.dev/

[rust-install]: https://www.rust-lang.org/tools/install

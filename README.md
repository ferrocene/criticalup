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
- Once the PR is approved, a reviewer will comment `@handlebors r+` to merge the PR.
- After the PR is merged, checkout `main` branch and update it (`git pull`) with the latest changes.
- Create a tag `git tag 'vX.Y.Z'`.
- Push the tag `git push origin vX.Y.Z`. This should trigger the release build in GitHub Actions and publish the release
  on its own.
- Create a new PR updating the version to `X.Y.(Z+1)-prerelease.1`, eg `1.5.0` would become `1.5.1-prelease.1`.

If the release build fails:

- Revert the changes from `release/vX.Y.Z` and open a PR to be merged to `main`.
- Delete the tag from GitHub.


## Verifying signatures

We use [`cosign`](https://github.com/sigstore/cosign) to verify signatures on Linux platforms.
Install cosign. Inside the archive, there is a <binary>.sigstore.json certificate.
Run:

```bash
cosign verify-blob <binary-name> \
    --certificate-identity-regexp ".*" \
    --bundle <binary-name>.sigstore.json \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```

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

## Docker image

Requires BuildKit builder backend.

`./Dockerfile` defines a `ferrocene_builder` target for downloading Ferrocene packages.

Such an image build with this target becomes useful in a multi-step build, serving final images that use the Ferrocene packages.

The ./Dockerfile copies a `criticalup.toml` configuration from the build context.

During the build, the image prints the `criticalup.toml` configuration being used. If you are unsure which configuration is being used, pass the `--no-cache --progress=plain` flags to the Docker build command to verify it.

### Build arguments

The following optional build arguments are available:

- `TARGET_UBUNTU_VERSION` — the Ubuntu version. Defaults to 24.04
- `CRITICALUP_RELEASE` — the `criticalup` release version. Defaults to the latest available version.

### Authentication

A `criticalup` authentication token is required. Pass it as the `criticalup_token` secret.

### Downloaded packages

The downloaded package tarballs are located in the image, under:

`/root/.cache/criticalup/artifacts/products/ferrocene/releases/...`

### Usage

#### Adquire a token

The [CriticalUp documentation](...) describes how to generate a `criticalup` token.

Assuming the `criticalup` token is stored in an environment variable named `CRITICALUP_TOKEN`:

#### Example image

The `ferrocene_builder` image is intended to be used as part of a multi-step build. At the end of the Dockerfile we
define an example target image that, when run, lists the downloaded Ferrocene packages.

```bash
docker build \
  --secret id=criticalup_token,env=CRITICALUP_TOKEN \
  -t example .

docker run example
```

[criticalup-docs]: https://criticalup.ferrocene.dev/

[rustup]: https://github.com/rust-lang/rustup

[ferrocene]: https://ferrocene.dev/

[rust-install]: https://www.rust-lang.org/tools/install

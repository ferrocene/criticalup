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


## Verifying signatures

We use [`cosign`](https://github.com/sigstore/cosign) to verify signatures on Linux platforms.
Install cosign. Inside the archive, there is a <binary>.sigstore.json certificate.
Run:

cosign verify-blob <binary-name> \
    --certificate-identity-regexp ".*" \
    --bundle <binary-name>.sigstore.json \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com


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

We provide ./docker/Dockerfile, defining an image `ferrocene_builder` that can be used download packages in a multi step/multi arch docker build.
Docker with Buidkit enabled is required.

If no configuration file is copied into the image in a modified Docker file definition (`ADD criticalup.toml .`), criticalup will initialize one.
On being built, the image prints out the used criticalup.toml. If doubts, pass the (`--no-cache --progress=plain`) flags to the build command  to confirm which configuration is being used.

The following build-args are available:

`FERROCENE_RELEASE`  a Ferrocene release version
`TARGET_UBUNTU_VERSION` an Ubuntu version
`CRITICALUP_RELEASE` a criticalup release version

Passing the criticalup secret token is required:
`criticalup_token` a criticalup token

The downloaded package tarballs are located in
` root/.cache/criticalup/artifacts/products/ferrocene/releases/...`

### usage

The [CriticalUp Documentation][criticalup-docs] Authenticating section describes how to generate the criticalup token.
Assuming we have the criticalup token in an env variable named CRITICALUP_TOKEN.

#### build example image

Build example image, that uses Ferrocene_builder image to copy Ferrocene packages from.

```bash
docker build --secret id=criticalup_token,env=CRITICALUP_TOKEN --build-arg FERROCENE_RELEASE=stable-26.02.0 . -t example
```


`docker run example` recursively lists the downloaded Ferrocene packages.

#### build ferrocene_builder image

Build only the ferrocene_builder image, and then define a Dockerfile that uses the image in a multi-step setup.

```bash
docker build --from ferrocene_builder --secret id=criticalup_token,env=CRITICALUP_TOKEN --build-arg FERROCENE_RELEASE=stable-26.02.0 . -t ferrocene_builder
```



[criticalup-docs]: https://criticalup.ferrocene.dev/

[rustup]: https://github.com/rust-lang/rustup

[ferrocene]: https://ferrocene.dev/

[rust-install]: https://www.rust-lang.org/tools/install

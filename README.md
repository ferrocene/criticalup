<!-- SPDX-FileCopyrightText: The Ferrocene Developers -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# CriticalUp

Criticalup is a toolchain manager for [Ferrocene][ferrocene], similar to [`rustup`][rustup].

> [!NOTE]  
>
> For normal usage and binary installation, please consult the [CriticalUp Documentation][criticalup-docs].

# Installing

Installation instructions for CriticalUp are included in [each release](https://github.com/ferrocene/criticalup/releases) as well as the [documentation][criticalup-docs].


# Development

CriticalUp only requires a working Rust and C toolchain to build. [Installation instructions][rust-install] for Rust typically include installing a C toolchain as well.

To build a debug version of the development-targeting CriticalUp:

```bash
cargo build -p criticalup-dev
```

To build a debug version of the production-targeting CriticalUp:

```bash
cargo build -p criticalup
```

To build a release version:

```bash
cargo build -p criticalup --release
```

To test CriticalUp:

```bash
cargo test
```

[criticalup-docs]: https://ferrocene.github.io/criticalup
[rustup]: https://github.com/rust-lang/rustup
[ferrocene]: https://ferrocene.dev/
[rust-install]: https://www.rust-lang.org/tools/install

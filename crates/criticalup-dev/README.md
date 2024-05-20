<!--
SPDX-FileCopyrightText: The Ferrocene Developers
SPDX-License-Identifier: MIT OR Apache-2.0
-->

`criticalup-dev`
----------------

A command line tool similar to `rustup` to manage installations of Ferrocene toolchains.

> [!NOTE]  
> The documentation here is primarily intended for developers of the `criticalup-dev` crate.
>
> Ferrocene users should refer to [the documentation][ferrocene-public-docs] for all their needs.

Installation
============

> [!NOTE]
> This repository is currently private, you need to use an SSO-authenticated SSH key or token along with the `--git` parameter.

```bash
cargo install --git ssh://git@github.com/ferrocene/criticalup.git criticalup-dev
criticalup-dev --help
```

Usage
=====

To authenticate with the portal:

```bash
criticalup-dev auth set
```

Then, enter a token obtained from the [Token page of the Customer Portal][customer-portal-tokens].

To check authentication status:

```bash
criticalup-dev auth
```

To install the toolchain specified by the `criticalup.toml` in the current working directory:

```bash
criticalup-dev install
```

[ferrocene-public-docs]: https://public-docs.ferrocene.dev/main/index.html
[customer-portal]: https://customers-dev.ferrocene.dev/
[customer-portal-tokens]: https://customers-dev.ferrocene.dev/users/tokens

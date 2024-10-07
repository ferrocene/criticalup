<!--
SPDX-FileCopyrightText: The Ferrocene Developers
SPDX-License-Identifier: MIT OR Apache-2.0
-->

`criticalup`
------------

A command line tool similar to `rustup` to manage installations of Ferrocene toolchains.

> [!NOTE]  
> The documentation here is primarily intended for developers of the `criticalup` crate.
>
> Ferrocene users should refer to [the documentation][ferrocene-public-docs] for all their needs.

Installation
============


```bash
cargo install --git ssh://git@github.com/ferrocene/criticalup.git criticalup
criticalup --help
```

Usage
=====

To authenticate with the portal:

```bash
criticalup auth set
```

Then, enter a token obtained from the [Token page of the Customer Portal][customer-portal-tokens].

To check authentication status:

```bash
criticalup auth
```

To install the toolchain specified by the `criticalup.toml` in the current working directory:

```bash
criticalup install
```

[ferrocene-public-docs]: https://public-docs.ferrocene.dev/main/index.html
[customer-portal]: https://customers.ferrocene.dev/
[customer-portal-tokens]: https://customers.ferrocene.dev/users/tokens

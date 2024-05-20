<!--
SPDX-FileCopyrightText: The Ferrocene Developers
SPDX-License-Identifier: MIT OR Apache-2.0
-->

`criticalup-cli`
----------------

A command line tool similar to `rustup` to manage installations of Ferrocene toolchains.

> [!NOTE]  
> The documentation here is primarily intended for developers of the `criticalup-cli` crate.
>
> Ferrocene users should refer to [the documentation][ferrocene-public-docs] for all their needs.

`criticalup-cli` is a library used for *whitelabel-able* binaries, and should not be directly installed.

In general, developers and users will use either [`criticalup`](../criticalup/) or [`criticalup-dev`](../criticalup-dev/).

This crate is *technically* installable, however the binary `criticalup-test` is only intended for the test suite and should not be used by developers or users.

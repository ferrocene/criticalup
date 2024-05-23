.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _platforms:

Platforms
=========

CriticalUp has support for multiple platforms.


Supported platforms
-------------------

While CriticalUp is not a qualified tool, it can be used to install qualified
toolchains. You must refer to the documentation for the version of Ferrocene
you are using to determine if a toolchain is qualified for safety-critical
contexts.

.. list-table::
   :header-rows: 1

   * - Target
     - Triple
     - Notes

   * - :target:`x86_64-unknown-linux-gnu`
     - ``x86_64-unknown-linux-gnu``
     - \-

   * - :target:`aarch64-apple-darwin`
     - ``aarch64-apple-darwin``
     - \-

   * - :target:`aarch64-unknown-linux-gnu`
     - ``aarch64-unknown-linux-gnu``
     - \-

   * - :target:`x86_64-pc-windows-msvc`
     - ``x86_64-pc-windows-msvc``
     - \-


If your project needs support for a target not listed here, please reach out to
the Ferrocene support team.

.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _criticalup_toml:

criticalup.toml
===============

CriticalUp's per-project configuration resides in a ``criticalup.toml``
manifest that is similar to a ``rust-toolchain.toml`` file
`(documentation) <https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file>`__.

A typical ``criticalup.toml`` manifest looks like this:

.. code-block::

    manifest-version = 1

    [products.ferrocene]
    release = "stable-24.05.0"
    packages = [
        "cargo-${rustc-host}",
        "rustc-${rustc-host}",
        "rust-std-${rustc-host}",
        "llvm-tools-${rustc-host}",
        "rust-src"
    ]

This manifest will install everything the average developer might want for a Ferrocene
based project.

Sample manifest
------------------------

You can create a sample manifest - ``criticalup.toml`` - using the ``init`` command.

.. code-block::

   criticalup init --release <RELEASE>

For more information, see :ref:`creating_sample_manifest` in the "Using CriticalUp" section.

Manifest Settings
-----------------


``manifest-version``
^^^^^^^^^^^^^^^^^^^^

The ``manifest-version`` specifies which version of the manifest format should be used.

Currently, only ``1`` is supported.

.. code-block::
    
    manifest-version = 1


``products``
^^^^^^^^^^^^

A map of ``product`` entries, as defined :ref:`in 'Product Settings' below
<product_settings>`.

.. note::
    
    Currently CriticalUp only supports one ``product`` entry, this is typically
    named ``ferrocene``.

    This will change in the future.

.. code-block::

    [products.ferrocene]
    release = "stable-24.05.0"
    packages = [
        "rustc-${rustc-host}",
        "rust-std-aarch64-unknown-none"
    ]


.. _product_settings:

Product Settings
----------------

``release``
^^^^^^^^^^^

The desired release of the product, releases are listed on the `release
channels page <https://releases.ferrocene.dev/ferrocene/index.html>`_.


.. code-block::

    [products.ferrocene]
    release = "stable-24.05.0"
    # ...

``packages``
^^^^^^^^^^^^

A set of package names as listed in the release page, for example the `stable-24.05.0
<https://releases.ferrocene.dev/ferrocene/files/stable-24.05.0/index.html>`_
release.

Each supported Ferrocene target lists required packages in the on its page
under "Compilation Targets" in the Ferrocene User Manual of the release. For
example, the `X86-64 Linux (glibc) target of the rolling release
<https://docs.ferrocene.dev/rolling/user-manual/targets/x86_64-unknown-linux-gnu.html#archives-to-install>`_.

.. code-block::

    [products.ferrocene]
    # ...
    packages = [
        "rustc-${rustc-host}",
        "rust-std-aarch64-unknown-none"
    ]

If ``${rustc-host}`` is present within a package name it is replaced with the
full host triple of the build host.

.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _toolchain_management:

Toolchain Management
====================

This chapter describes how to manage toolchains using CriticalUp.


The examples in this chapter assume the following directory structure:

.. code-block::

   .
   └── project
      └── criticalup.toml

Where the ``criticalup.toml`` contains the following content:

.. code-block::

   manifest-version = 1

   [products.ferrocene]
   release = "stable-25.02.0"
   packages = [
       "cargo-${rustc-host}",
       "rustc-${rustc-host}",
       "clippy-${rustc-host}",
       "rust-std-${rustc-host}",
       "rustfmt-${rustc-host}",
   ]

CriticalUp understands ``${rustc-host}`` to mean the target triple of the host operating system. These triple values are listed in :ref:`Platforms <platforms>`.

.. note::

   Options for ``criticalup.toml`` are detailed in :ref:`the reference <criticalup_toml>`.

Creating a ``criticalup.toml``
-----------------------------------

You can create a ``criticalup.toml`` using the ``init`` command.

.. code-block::

   criticalup init --release "stable-25.02.0"

For more information, see :ref:`creating_a_manifest` in the "Using CriticalUp" section.

.. _install_toolchain:

Installing Toolchains
^^^^^^^^^^^^^^^^^^^^^

After :ref:`installing CriticalUp <install>` and
:ref:`authenticating <authenticate>` CriticalUp is ready to manage
toolchains.

You can can change directory into the project and install the required
toolchain.

.. code-block::

   cd project
   criticalup install

To change the installed products, edit the ``criticalup.toml`` as desired. For example:

.. code-block::

    manifest-version = 1

   [products.ferrocene]
   release = "stable-25.02.0"
   packages = [
       "cargo-${rustc-host}",
       "rustc-${rustc-host}",
       "clippy-${rustc-host}",
       "rust-std-${rustc-host}",
       "rustfmt-${rustc-host}",
       "rust-std-aarch64-unknown-none", # Line added
   ]

Then run the install command again:


.. code-block::

   criticalup install

When an internet connection is not available, a previously fetched package
can be installed without using the network by passing the ``--offline`` flag.

Removing Toolchains
^^^^^^^^^^^^^^^^^^^

An installation can be removed by running the ``criticalup remove`` command
from the directory containing the ``criticalup.toml``:

.. code-block::

   criticalup remove

Cleaning Unused Toolchains
^^^^^^^^^^^^^^^^^^^^^^^^^^

Over time CriticalUp's stored installations or cache may accumulate artifacts
that are no longer used. If CriticalUp's state directory begins to consume too
much disk space the ``clean`` command can help by deleting unused toolchains.


.. code-block::

   criticalup clean

Verifying Toolchains
^^^^^^^^^^^^^^^^^^^^

If a toolchain is suspected to be corrupted or tampered with, the verification
step performed during installation can be repeated.

From the direcory containing the relevant ``criticalup.toml``:

.. code-block::

   criticalup verify

Creating Archives of Toolchains
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

CriticalUp can produce uncompressed tarballs of toolchains which can then be
placed in backups.

.. code-block::

   criticalup archive out.tar

If an output path is omitted, ``criticalup archive`` emits the archive to
stdout.

When an internet connection is not available, a previously fetched package
can be tarballed without using the network by passing the ``--offline`` flag.

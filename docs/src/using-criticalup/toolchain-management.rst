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
   release = "nightly-2024-04-03"
   packages = [
      "rustc-x86_64-unknown-linux-gnu",
      "cargo-x86_64-unknown-linux-gnu",
   ]

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

To change the installed products, edit the ``criticalup.toml`` as desired, for example:

.. code-block::

   manifest-version = 1

   [products.ferrocene]
   release = "nightly-2024-04-03"
   packages = [
      "rustc-x86_64-unknown-linux-gnu",
      "cargo-x86_64-unknown-linux-gnu",
      "rustfmt-x86_64-unknown-linux-gnu", # Line added
   ]

Then run the install command again:


.. code-block::

   criticalup install

Removing Toolchains
^^^^^^^^^^^^^^^^^^^

An installation can be removed by running the ``criticalup remove`` command
from the directory containing the ```criticalup.toml```:

.. code-block::

   cd project
   criticalup remove

Cleaning Unused Toolchains
^^^^^^^^^^^^^^^^^^^^^^^^^^

Over time CriticalUp's stored installations may accumulate artifacts that
are no longer used. If CriticalUp's state directory begins to consume too much
disk space the ``clean`` command can help by deleting unused toolchains.


.. code-block::

   criticalup clean
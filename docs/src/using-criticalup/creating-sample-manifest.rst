.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _creating_sample_manifest:

Creating sample manifest
========================

This chapter describes a way to quickly create a sample ``criticalup.toml`` for your project.

Creating a sample manifest file
-------------------------------

Running the ``init`` command will create a file ``criticalup.toml`` in the current working directory.

.. code-block::

   criticalup init --release <RELEASE>


The ``--release`` flag's value must be one of the product releases listed on the `release
channels page <https://releases.ferrocene.dev/ferrocene/index.html>`_.

Running this command again will overwrite the existing ``criticalup.toml`` file in the current working directory.

Print the contents only
-----------------------

Running the ``init`` command with flag ``--print-only`` will print the sample manifest contents to stdout. It will not create nor write any file to the file system.

.. code-block::

   criticalup init --release <RELEASE> --print-only

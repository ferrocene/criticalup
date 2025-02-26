.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _creating_a_manifest:

Creating a manifest
========================

This chapter describes a way to quickly create a ``criticalup.toml`` for your project.

Creating a manifest file
-------------------------------

Running the ``init`` command will create a ``criticalup.toml`` in the current working directory.

.. code-block::

   criticalup init --release <RELEASE>


The ``--release`` flag's value must be one of the product releases listed on the `release
channels page <https://releases.ferrocene.dev/ferrocene/index.html>`_. For example,

.. code-block::

    criticalup init --release "stable-25.02.0"

.. caution::

    Running this command again will overwrite the existing ``criticalup.toml`` file in the current working directory.

Print the contents only
-----------------------

Running the ``init`` command with flag ``--print`` will print the sample manifest contents to stdout. It will not create nor write any file to the file system.

.. code-block::

   criticalup init --release <RELEASE> --print

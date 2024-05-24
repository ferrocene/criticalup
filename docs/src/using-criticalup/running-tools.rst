.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _running_tools:

Running Tools
=============

This chapter describes how to run specific tools using CriticalUp.

The examples in this chapter assume the following directory structure:

.. code-block::

   .
   └── project
      └── criticalup.toml

**Prerequisites**

* :ref:`Installation - CriticalUp <install>`
* :ref:`Authentication <authenticate>`
* :ref:`Installation - Toolchain <install_toolchain>`

After installing CriticalUp, authenticating, and installing a toolchain,
CriticalUp can be used to run the specified tools from the installed toolchain.

CriticalUp creates a set of *binary proxies* for tools which it has installed.
Which discover the relevant ``criticalup.toml`` and executes the correct
version of the tool.

.. note::

   If CriticalUp does not find a ``criticalup.toml`` in the current directory,
   it will search the parent directory, then the parent of that, up to the root
   directory of the system.

.. code-block::

   cd project
   criticalup run rustc --help


Locating Tools
^^^^^^^^^^^^^^

We can find the true path of a tool for the current toolchain with the ``which`` command:


.. code-block::

   cd project
   criticalup which rustc

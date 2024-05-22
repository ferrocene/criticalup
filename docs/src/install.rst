.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _install:

Installing CriticalUp
=====================

This chapter describes how to install CriticalUp.

Before proceeding, you should identify the :doc:`platform <../platforms>` you
want to install onto. You must pick the platform of the host you're going to
install CriticalUp on.

Based on the platform you chose, you must follow the directions for the
relevant operating system. Installation and usage does not differ between
architectures unless otherwise noted.

Linux
-----

From a terminal run:

.. code-block::

   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ferrocene/criticalup/releases/download/criticalup-cli-v0.0.0/criticalup-cli-installer.sh | sh

CriticalUp will install into ``$XDG_DATA_HOME/criticalup``, or if that
environment variable is not set, ``$HOME/.local/share/criticalup``.


MacOS
-----

From a terminal run:

.. code-block::

   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ferrocene/criticalup/releases/download/criticalup-cli-v0.0.0/criticalup-cli-installer.sh | sh

CriticalUp will install into ``$HOME/Library/Application Support/criticalup``.


Windows
-------

From a terminal run:

.. code-block::

   powershell -c "irm https://github.com/ferrocene/criticalup/releases/download/criticalup-cli-v0.0.0/criticalup-cli-installer.ps1 | iex"

CriticalUp will install into ``{FOLDERID_RoamingAppData}``, usually ``%appdata%\\criticalup``.
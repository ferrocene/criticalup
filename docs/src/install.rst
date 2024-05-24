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

   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ferrocene/criticalup/releases/latest/download/criticalup-installer.sh | sh

This will install CriticalUp into ``$HOME/.cargo/bin/criticalup``.

CriticalUp will create a state directory where it installs toolchains in ``$XDG_DATA_HOME/criticalup``, or if that
environment variable is not set, ``$HOME/.local/share/criticalup``.

To uninstall, remove ``$HOME/.cargo/bin/criticalup`` then delete the state directory.

MacOS
-----

From a terminal run:

.. code-block::

   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ferrocene/criticalup/releases/latest/download/criticalup-installer.sh | sh

This will install CriticalUp into ``$HOME/.cargo/bin/criticalup``.

CriticalUp will create a state directory where it installs toolchains in ``$HOME/Library/Application Support/criticalup``.

To uninstall, remove ``$HOME/.cargo/bin/criticalup`` then delete the state directory.

Windows
-------

We recommend using the MSI installation method. If you'd prefer not to use the MSI, a powershell based install is available.

MSI based
^^^^^^^^^

The recommended method of installation is to `download and run the MSI <https://github.com/ferrocene/criticalup/releases/latest/download/criticalup-x86_64-pc-windows-msvc.msi>`_.

This will install CriticalUp into ``C:\Program Files\criticalup`` by default, but the location is configurable in the installer interface.

CriticalUp will create a state directory where it installs toolchains in
``{FOLDERID_RoamingAppData}``, usually ``%appdata%\criticalup``.

To uninstall, use the Windows Add/Remove programs interface or run ``winget remove criticalup``, then delete the state directory.

Powershell based
^^^^^^^^^^^^^^^^

From a terminal run:

.. code-block::

   powershell -c "irm https://github.com/ferrocene/criticalup/releases/latest/download/criticalup-installer.ps1 | iex"

This will install CriticalUp into ``$HOME/.cargo/bin/criticalup``.

CriticalUp will create a state directory where it installs toolchains in
``{FOLDERID_RoamingAppData}``, usually ``%appdata%\criticalup``.

To uninstall, remove ``$HOME/.cargo/bin/criticalup`` then delete the state directory.
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

After :ref:`installing CriticalUp <install>`,
:ref:`authenticating <authenticate>`, and :ref:`installing a toolchain
<install_toolchain>`, CriticalUp can be used to run the specified tools
from the installed toolchain.

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

It is possible to find the absolute path of a tool for the current toolchain with the ``which``
command:


.. code-block::

   cd project
   criticalup which rustc


Using the Binary Proxies
^^^^^^^^^^^^^^^^^^^^^^^^

CriticalUp creates a number of 'binary proxies' which can be used to run the appropriate Ferocene
binaries for a given workspace. These can be added to your shell path on any OS.

It's important to note that these binaries share the same binary names as any Rust toolchain that
may already be installed. If you already have Rust installed (for example, via ``rustup``) you
may wish either to remove it, use Ferrocene via ``criticalup run``, or remove the upstream Rust
toolchain from your path and use ``rustup run`` to access it.

Linux
-----

Proxies are located at ``$XDG_DATA_HOME/criticalup/bin``, typically this is
``~/.local/share/criticalup/bin/``.

You can add the following line to your ``~/.bashrc`` or ``~/.zshrc`` to add the binary proxies to
your ``PATH``:

.. code-block::

   export PATH="$PATH:$HOME/.local/share/criticalup/bin"

If you're using a different shell, such as
`nushell <https://www.nushell.sh/book/configuration.html#path-configuration>`, you may need to
consult the shell's documentation on how to add to the path.

macOS
-----

Proxies are located at ``~/Library/Application Support/criticalup/bin/``. 

You can add the following line to your ``~/.zshrc`` to add the binary proxies to your ``PATH``:

.. code-block::

   export PATH="$PATH:$HOME/Library/Application Support/criticalup/bin"

If you're using a different shell, such as
`nushell <https://www.nushell.sh/book/configuration.html#path-configuration>`, you may need to
consult the shell's documentation on how to add to the path.

Windows
-------

Proxies are located at ``%appdata%\criticalup\bin\``:

On Windows 11, you can add the folder to your system path by hitting the Windows key and searching 
'Edit environment variables for your account', then selecting the control panel. If you can't find
it, you can use the 'Run' dialog to directly launch it: 

.. code-block::

   rundll32.exe sysdm.cpl,EditEnvironmentVariables

Once there, edit the ``PATH`` variable to include the following entry:

.. code-block::

   %USERPROFILE%\AppData\Roaming\criticalup\bin\

You'll then need to sign out, and back in for changes to take effect.

If you're using a different shell, such as
`nushell <https://www.nushell.sh/book/configuration.html#path-configuration>`, you may need to
consult the shell's documentation on how to add to the path.

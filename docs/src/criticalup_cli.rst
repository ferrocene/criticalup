.. SPDX-License-Identifier: MIT OR Apache-2.0
   SPDX-FileCopyrightText: The Ferrocene Developers

CriticalUp Command-Line Interface
=================================

.. cli:program:: criticalup

    .. code-block::

        $ criticalup [OPTIONS] <COMMAND>

    **GLOBAL OPTIONS**

    .. cli:option:: -V --version

        Prints the version of CriticalUp.

        Example:

        .. code-block::

            $ criticalup -V

    .. cli:option:: -h --help

        Prints the help messages for commands and options to stdout.

    **COMMANDS**

    .. cli:subcommand:: auth [OPTIONS] <COMMAND>

        Authenticates or shows status of authentication with the download server.

        .. cli:subcommand:: set <TOKEN>

            Sets the authentication token used to interact with the download server.

            .. code-block::

                $ criticalup auth set <TOKEN>

        .. cli:subcommand:: remove

            Removes the authentication token used to interact with the download server.

            .. code-block::

                $ criticalup auth remove

    .. cli:subcommand:: install [OPTIONS]

        Installs the toolchain for the given project based on the project manifest ``criticalup.toml``.

        .. note::

            This command requires successful authentication (``criticalup auth``).

        .. cli:option:: --project <PROJECT>

            Option to provide path to the project manifest ``criticalup.toml``. If not provided,
            CriticalUp tries to find the project manifest in current and parent directories, recursively.
            An error is shown to the user if it fails to find the project manifest in any of the
            directories.

            Without ``--project``:

            .. code-block::

                $ criticalup install

            With ``--project``:

            .. code-block::

                $ criticalup install \
                    --project /path/to/manifest/criticalup.toml

        .. cli:option:: --reinstall

            Installs products, that may have already been installed.

            By default, CriticalUp does not install a product and its packages if they are
            already installed. This option overrides that behavior and installs the toolchain again.

            .. code-block::

                $ criticalup install --reinstall

                $ criticalup install --reinstall \
                    --project /path/to/manifest/criticalup.toml

        .. cli:option:: --offline

            Uses previously cached artifacts to install the product.

            By default, CriticalUp needs the download server to fetch the artifacts. This option does
            not contact the download server and uses locally cached artifacts for installation.

            If the local cache does not have the artifacts, an error is shown to the user.

            .. code-block::

                $ criticalup install --reinstall --offline

                $ criticalup install --reinstall --offline \
                    --project /path/to/manifest/criticalup.toml

    .. cli:subcommand:: remove [OPTIONS]

        Removes all the products artifacts specified in the manifest ``criticalup.toml`` from the
        CriticalUp state.

        .. caution::
            This does **not** delete the artifacts from the disk.

        .. cli:option:: --project <PROJECT>

            Option to provide path to the project manifest ``criticalup.toml``. If not provided,
            CriticalUp tries to find the project manifest in current and parent directories, recursively.
            An error is shown to the user if it fails to find the project manifest in any of the
            directories.

            Without ``--project``:

            .. code-block::

                $ criticalup remove

            With ``--project``:

            .. code-block::

                $ criticalup remove \
                    --project /path/to/manifest/criticalup.toml

    .. cli:subcommand:: clean [OPTIONS]

        Deletes all unused and untracked installations from the disk. This command is usually
        run after the ``remove`` command.

        When you install a product using a project manifest, the local CriticalUp state tracks those.
        This saves on disk-space by re-using the product and artifacts combination for various projects.
        If a set of artifacts are not used by any project, this command then deletes it from the state and
        disk.

    .. cli:subcommand:: which [OPTIONS] <BINARY>

        Displays which binary will be run for a given command.

        For example, let's assume the <BINARY> here is ``rustc``.

        .. cli:option:: --project <PROJECT>

            Option to provide path to the project manifest ``criticalup.toml``. If not provided,
            CriticalUp tries to find the project manifest in current and parent directories, recursively.
            An error is shown to the user if it fails to find the project manifest in any of the
            directories.

            Without ``--project``:

            .. code-block::

                $ criticalup which rustc

            With ``--project``:

            .. code-block::

                $ criticalup which rustc \
                    --project /path/to/manifest/criticalup.toml

    .. cli:subcommand:: run [OPTIONS] <COMMAND>

        Runs a command for a given toolchain.

        .. note::

            If the <COMMAND> has its own arguments and options, they can be passed as well.

        .. caution::

            This command/binary must be installed. CriticalUp will show an error if the binary cannot
            be found.

        For example, let's assume the <COMMAND> here is ``rustc``. Note that we could pass the option
        ``--version`` to ``rustc``.

        .. cli:option:: --project <PROJECT>

            Option to provide path to the project manifest ``criticalup.toml``. If not provided,
            CriticalUp tries to find the project manifest in current and parent directories, recursively.
            An error is shown to the user if it fails to find the project manifest in any of the
            directories.

            Without ``--project``:

            .. code-block::

                $ criticalup run rustc --version

            With ``--project``:

            .. code-block::

                $ criticalup run \
                    --project /path/to/manifest/criticalup.toml \
                    rustc --version

.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _docker:

Example usage in Docker files
=============================

This chapter presents an example on how configure criticalup in a dockerfile, to be used in a multi-stage build.
It requires a valid **CRITICALUP_TOKEN**

.. code-block:: docker

    FROM ubuntu AS base

    WORKDIR /app

    #  Get the binary from the releases page https://github.com/ferrocene/criticalup/releases
    COPY criticalup /app/
    COPY criticalup.toml /app/

    RUN --mount=type=secret,id=criticalup_token,env=CRITICALUP_TOKEN <<-EOF
       echo $CRITICALUP_TOKEN
      ./criticalup auth set $CRITICALUP_TOKEN
      ./criticalup install
      ./criticalup auth remove
    EOF

    CMD ["sh", "-c", "echo 'This image is intended for multi-stage builds only.' >&2; exit 1"]

    # This example copies the downloaded packages and lists them
    FROM ubuntu AS example1

    COPY --from=base /root/.cache/criticalup/artifacts artifacts
    CMD ["/usr/bin/ls", "-R", "artifacts"]

    # This example archives the installation to an tar file
    FROM base AS example2
    RUN ./criticalup archive --offline > archive.tar
    RUN tar -tvf archive.tar


This can be built running:

.. code-block:: bash

   CRITICALUP_TOKEN=<your token> docker build --secret id=criticalup_token,env=CRITICALUP_TOKEN .

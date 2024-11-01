.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

================================
CriticalUp Documentation
================================

.. raw:: html

   <p align="center"><a href="https://criticalup.ferrocene.dev">Read the
   specification &raquo;</a></p>

The CriticalUp Documentation (CUD) is a document describing the CriticalUp
tool.

The CriticalUp Documentation text is licensed under either the ``MIT``
or ``Apache-2.0`` licenses, at your option. Individual files might have
different licensing. Licensing metadata is present in each file, and the full
licenses text is present in the ``LICENSES/`` directory.

Building the specification
==========================

First, ``install <https://docs.astral.sh/uv/getting-started/installation/>``_
``uv`` if you haven't. ``uv`` is to Python what ``cargo`` is to Rust.

CUD uses `Sphinx`_ to build a rendered version of the specification. To
simplify building the rendered version, we created a script called ``make.py``
that takes care of installing the expected Sphinx release and invoking it with
the right flags.

You can build the rendered version by running::

   uv run ./make.py

By default, Sphinx uses incremental rebuilds to generate the content that
changed since the last invocation. If you notice a problem with incremental
rebuilds, you can pass the ``-c`` flag to clear the existing artifacts before
building::

   uv run ./make.py -c

The rendered version will be available in ``build/html/``.

You can also start a local server on port 8000 with automatic rebuild and
reload whenever you change a file by passing the ``-s`` flag::

   uv run ./make.py -s

Checking links consistency
==========================

It's possible to run Rust's linkchecker tool on the rendered documentation, to
see if there are broken links. To do so, pass the ``--check-links`` flag::

   uv run ./make.py --check-links

This will clone the source code of the tool, build it, and execute it on the
rendered documentation.

.. _Sphinx: https://www.sphinx-doc.org

Updating dependencies
=====================

We use ``pyproject.toml`` and ``uv lock`` to manage dependency versions.

To upgrade dependencies:

   uv lock --upgrade
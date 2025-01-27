.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _authenticate:

Authenticating
==============

This chapter describes how to authenticate CriticalUp on the
`Ferrocene Customer Portal`_.

You need a Ferrocene subscription to authenticate CriticalUp and download Ferrocene.

.. _Ferrocene Customer Portal: https://customers.ferrocene.dev/

After :ref:`installing CriticalUp <install>`, authenticate by running the ``auth set`` subcommand:

.. code-block::

   criticalup auth set

Follow the on-screen instructions to generate a new token, then paste the token
into the prompt. CriticalUp will validate the token.


Check Authentication Status
^^^^^^^^^^^^^^^^^^^^^^^^^^^

Review authentication and token state using the ``auth`` command.

.. code-block::

   criticalup auth


Unauthenticating
^^^^^^^^^^^^^^^^

To remove the authenticated token, run ``auth remove``.

.. code-block::

   criticalup auth remove
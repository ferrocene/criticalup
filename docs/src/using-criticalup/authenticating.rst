.. SPDX-FileCopyrightText: The Ferrocene Developers
.. SPDX-License-Identifier: MIT OR Apache-2.0

.. _authenticate:

Authenticating
==============

This chapter describes how to authenticate CriticalUp on the
`Ferrocene Customer Portal`_.

The example assumes you have a preexisting Ferrocene account. This example
presumes no existing directory structure.

.. _Ferrocene Customer Portal: https://customers.ferrocene.dev/

After :ref:`installing CriticalUp <install>` it's possible to authenticate
CriticalUp via the ``auth set`` subcommand.


.. code-block::

   criticalup auth set

Follow the on-screen instructions to generate a new token, then paste the token
into the prompt. CriticalUp will validate that the provided token is valid.


Check Authentication Status
^^^^^^^^^^^^^^^^^^^^^^^^^^^

The current authentication state and token used can be reviewed with the ``auth`` command.

.. code-block::

   criticalup auth


Unauthenticating
^^^^^^^^^^^^^^^^

In order to remove the authenticated token, the ``auth remove`` command can be used.

.. code-block::

   criticalup auth remove
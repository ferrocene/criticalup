.. _authenticate:

Authenticating
==============

This chapter describes how to authenicate CriticalUp with on the
`Ferrocene Customers Portal`_.

The example assumes you have a preexisting Ferrocene account. This example
presumes no existing directory structure.

.. _Ferrocene Customers Portal: https://customers.ferrocene.dev/

After :ref:`installing CriticalUp <install>` it's possible to authenticate
CriticalUp via the ``criticalup auth set`` subcommand.


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
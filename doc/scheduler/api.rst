.. default-domain:: lua


=============
Scheduler API
=============


Overview
========

.. warning:: API is still unstable and is subject to change

Scheduler is a lua_ script. All the API are exposed through functions on the
main module.

Callbacks
=========

Functions that verwalter calls on its own.

.. note:: You can use coroutines inside the code, but you can't ``yield``
   to rust code. I.e. the code is always synchronous and must return the
   value on each call. However, you can store some custom state in the schedule
   itself.

.. function:: schedule(named_arguments)

   :param peers: List of peers and pings to them as reported by cantal
   :param roles: Metadata for roles stored in `/etc/verwalter`
   :param parents: List of parent schedules (the ones that are active now).
     Usually there is only one. But when we join cluster just after split-brain
     there can be more than one parent schedule



.. _lua: https://www.lua.org/

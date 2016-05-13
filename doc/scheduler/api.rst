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
   :param runtime: Metadata stored in `/etc/verwalter/runtime`
   :param parents: List of parent schedules (the ones that are active now).
     Usually there is only one. But when we join cluster just after split-brain
     there can be more than one parent schedule

   Return value of the scheduler is a JSON object with the following keys:

   vars
     Mapping (json object) that contains arbitrary variables which will be
     passed to the renderer. They might be overriden by role and node-specific
     variables. See below.

     Example::

        {"vars": {
            "cluster_name": "dev"}}

   roles
     Mapping of role to vars of this role. This contains variables common for
     specific role on all nodes. All roles specified here will be rendered
     on all machines (can spawn ``0`` instances, though).

     Example::

        {"roles": {
            "django": {
                "version": "v0.1.3",
                "listen-ports": "8080"}}

   nodes
     Mapping of node name (short/unqualified hostname) to node metadata.
     Each node contains: `vars` and `roles`.

     Example:

         {"nodes": {
             "alpha": {
                 "vars": {"nearest_cache_addr": "slave7.redis.local"},
                 "roles": {
                     "django": {
                         "instances": 1,
                         "version": "v0.1.3"}}}}}

    More information on how variables are composed for the renderer is
    in :ref:`Rendering` docs.





.. _lua: https://www.lua.org/

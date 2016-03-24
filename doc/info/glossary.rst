========
Glossary
========

.. glossary::

  configuration
    The initial input to the verwalter's scheduler. It conists of:

    * All data in ``/etc/verwalter/runtime/*``
    * All templates and actions in ``/etc/verwalter/templates/*``

    It's expected that these files are never mutated. But new ones might be
    added. E.g. if there is ``runtime/v1.0.1/..`` and new version
    ``runtime/v1.0.2`` appears verwalter reads it as fast as possible, and
    makes it available on next scheduler run.

    All configuration versions are read by verwalter. So you can write any
    required logic in scheduler. For example, to arrange a blue/green
    deployment strategy you may need to keep "blue" configuration around even
    when no processes running it are present.

  schedule
    A data structure that holds information about all the services that must
    run on the whole cluster. This is the result of running a scheduler code.

    In fact it's just a piece of JSON-like data, which you may use in templates
    when rendering the configurations. It may contain anything, but usually
    it's something along lines of nested dicts:
    ``host-name -> process-name -> number-of-instances``.

  scheduler
    The Lua code that receives a *configuration* and a *state* and generates
    a *schedule*. Basically it's just a (pure) function.

    A scheduler may do whatever it needs for the transformation. But, but it's
    very important to obey the following rules:

    1. No external data should be used. Just *configuration* and *state*.
    2. No side effects allowed, like writing to the files or even reading
       current date/time (we provide date/time as part of state, though)
    3. It shouldn't be too slow

  deployment id
    The unique identifier of the series of the actions that was run to apply
    certain config. Deployment id is local for single machine, but may span
    across roles. Single deployment id is used only once, so they refer to
    the time range when deployment started and finished. Multiple deployments
    can't be run on single machines simultaneously.

    Not all roles can be deployed with the single deployment id just the ones
    which need an update. Each role may execute commands only once during
    single deployment.

    There is no direct correspondence between config hash and deployment id.
    Single config may be deployed multiple times even on single machine.
    (each time when verwalter is restarted, each time when config changed and
    then rolled back again). But single deployment may deploy only single
    configuration. I.e. configuration can't change during deployment.

    And there is no direct match between application update and deployment id.
    The (rolling) application id usually involves multiple configuration
    updates.  And each configuration update triggers one deployment on each
    machine.  Also multiple rolling updates of different applications may take
    place at the same time. And all of them correspond to a single
    configuration change at any point in time.

  role
    A single deployment unit. A role has it's own configuration independent
    of others(set of versions of containers, set of config templates).

    A role may contain multiple containers. And multiple different setups on
    different nodes. It's up to a lua configuration.

    Usually single role refers to single "sandbox" in lithos_, but this limit
    is not enforced.

    Similarly blue/green deploy (or rolling update) between versions is
    usually performed for a role. Which means each role has it's own state of
    the deployment, and multiple roles can be migrated independently. But this
    is not enforced either. With careful scripting you can do both:
    synchronize updates of multiple roles or update different processes in
    single role using some independent states.


.. _lithos: http://github.com/tailhook/lithos

==============================
Configuration Directory Layout
==============================

The layout of /etc/verwalter directory.

The directory layout is still in flux. Here are somewhat current draft.

* ``scheduler`` -- scheduler code in lua

    * ``scheduler/SCHEDULER_VERSION/main.lua`` -- the entry point of the
      scheduler (``scheduler`` function) [1]_
    * ``scheduler/SCHEDULER_VERSION/**/*.lua`` -- other files that are
      ``require``d from scheduler

* ``templates`` -- the templates to render configuration locally

    * ``templates/ROLE/TMPL_VERSION`` -- templates for role and version [1]_

        * ``**/*.hbs`` -- bare configuration templates
        * ``**/*.vw.yaml`` -- instructions on how to apply the template

* ``runtime`` -- the runtime metadata, mostly list of processes to run and
  other data needed for scheduling. Basically all of this is passed to the
  scheduler

    * ``runtime/ROLE/ROLE_VERSION`` -- metadata dir for role and version

        * ``NAME.yaml`` -- adds some metadata under key ``NAME``
        * ``NAME.json`` -- just another format of the same thing

* ``machine`` -- the current machine metadata

    * ``NAME.yaml/json`` -- adds some metadata under key ``NAME``

* ``frontend`` -- the files to render the frontend [2]_

    * ``common/*`` -- common files for the whole cluster (e.g. libraries)
    * ``ROLE/*`` -- role-specific things [3]_

.. note:: We avoid the term "application" here because it's inherently vague.
   The role is just unit that may be deployed independendly (so it's also
   versioned independently). The role may consists multiple applications or
   application may be built on top of multiple roles, dependening on use
   case and how you define the application.

.. [1] The version of scheduler and version of templates is not the same as
   version of role (i.e. an application). It's expected that scheduler and
   templates change very rarely and only by admins, not by release managers.
   Also you might use "shadow" scheduler and "shadow" template renderer for
   debugging.

.. [2] Each installation have different needs. So verwalter doesn't have a
   frontend that is packaged with verwalter. We only provide the API, and a
   default (or example) frontend which you might use as a starting point. Sure
   verwalter serves static files so you don't need to install a separate web
   server.

.. [3] We don't have frontend files versioned yet. It's not critical part of
   the system and it assumed that an (updated) frontend should support at
   least few versions of the application (role).


Deployment
==========

It's assumed that ``scheduler`` and ``templates`` are written by SysOps. They
should be versioned in version control system and deployed as needed.

The ``frontend`` is very similar. It should be versioned too. It's only
mentioned separately because usually changed by some frontender or release
engineer or whatever.

The ``runtime`` folder is assumed to be deployed by buildbot. I.e. when build
is done, buildbot does two things to prepare deployment:

1. Upload built image to all servers that will be able to run the application
2. Put app metadata in the ``runtime`` folder on same machines

Then it's up to the scheduler if it deploys the version automatically or waits
for operator to trigger the update action.

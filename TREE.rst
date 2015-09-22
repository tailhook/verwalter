==============================
Configuration Directory Layout
==============================

The layout of /etc/verwalter directory.

The directory layout is still in flux. Here are somewhat current draft.

* ``scheduler`` -- scheduler code in lua
    * ``scheduler/main.lua`` -- the entry point of the scheduler (``scheduler``
      function)
    * ``scheduler/**/*.lua`` -- other files that are ``require``d from
      scheduler

* ``templates`` -- the templates to render configuration locally
    * ``templates/APPNAME/TMPL_VERSION`` -- templates for app and version [1]_
        * ``**/*.hbs`` -- bare configuration templates
        * ``**/*.vw.yaml`` -- instructions on how to apply the template

* ``runtime`` -- the runtime metadata, mostly list of processes to run and other
  data needed for scheduling. Basically all of this is passed to the scheduler
    * ``runtime/APPNAME/APP_VERSION`` -- metadata dir for app and version
        * ``NAME.yaml`` -- adds some metadata under key ``NAME``
        * ``NAME.json`` -- just another format of the same thing


.. [1] The version of templates is not the same as version of application. It's
   expected that templates will change very rarely and only by admins, not by
   release managers.


Exit Codes
==========

* ``3`` -- initial configuration read failed
* ``4`` -- failed to load scheduler's lua code
* ``91`` -- killed by watchdog of scheduler, which means scheduler has not
  finished it's work within one second, or scheduler lua scripts
  could not be initialized within ten seconds
* ``92`` -- scheduler thread have panicked (probaby a bug)
* ``93`` -- killed by watchdog of the render/apply code. This probably means
  either your templates are a way too slow, or commands that are
  used to apply config are doing too much work. We currently have
  a fixed timeout of 180 seconds (3 min) for all of the stuff there
  (normally it's done in a fraction of second)
* ``94`` -- the thread that applies config have panicked (probably a bug)

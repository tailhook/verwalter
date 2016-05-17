Exit Codes
==========


Verwalter Daemon
================

* ``3`` -- initial configuration read failed
* ``4`` -- failed to load scheduler's lua code
* ``5`` -- failed to add inotify watch
* ``91`` -- killed by watchdog of scheduler, which means:

    * scheduler has not finished it's work within one second
    * scheduler lua scripts could not be initialized within ten seconds
    * "runtime" metadata could not be loaded within 2 seconds
    * inotify continuously reports changes during 10 seconds

* ``92`` -- scheduler thread have panicked (probaby a bug)
* ``93`` -- killed by watchdog of the render/apply code. This probably means
  either your templates are a way too slow, or commands that are
  used to apply config are doing too much work. We currently have
  a fixed timeout of 180 seconds (3 min) for all of the stuff there
  (normally it's done in a fraction of second)
* ``94`` -- the thread that applies config have panicked (probably a bug)


Verwalter Render
================

This may be visible in verwalter's deployment log:

* ♻ ``2`` -- argparse error, should not happen, but may be if version of
  verwalter-render (on disk) doesn't match verwalter daemon running
* ♻ ``3`` -- error validating arguments, should be treated same as ``2``
* ♻ ``5`` -- verwalter daemon is running different version from
  verwalter-render. This probably means you should restart verwalter daemon.
  For other things it should be treated same as ``2``
* ♻ ``4`` -- no ``template`` key found in metadata, this means scheduler
  returned incomplete data for this role
* ♻ ``10`` -- error when reading or rendering templates
* ``20`` -- error appling templates (executing commands)
* ``81`` -- error when doing logging, this probably means that some errors are
  absent in logs

The error codes marked with ``♻`` mean that no actual rendering process is
started. I.e. system is consistent (old) state. With other codes we can't
easily say whether configuration was appllied partial, comprehensively or not
at all.

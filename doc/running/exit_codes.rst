Exit Codes
==========

* ``3`` -- initial configuration read failed
* ``4`` -- failed to load scheduler's lua code
* ``91`` -- killed by watchdog of scheduler, which means scheduler has not
            finished it's work within one second, or scheduler lua scripts
            could not be initialized within ten seconds
* ``92`` -- scheduler thread is dead for some reason

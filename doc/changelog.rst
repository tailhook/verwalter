Verwalter Changes by Version
============================

.. _changelog-0.10.3:

Verwalter 0.10.3
----------------

* bugfix: timestamps in peer info now serialize as milliseconds since epoch

.. _changelog-0.10.2:

Verwalter 0.10.2
----------------

* feature: upgrading trimmer to 0.3.6 allows to use escaping, dict and list
  literals in (.trm) templates
* Using ``wasmi`` instead of ``parity-wasm`` for interpreting wasm
* Initial routing for alternative frontends (``/~frontend-name/...`` urls)


.. _changelog-0.10.1:

Verwalter 0.10.1
----------------

* Timeout for incoming requests changed 10sec -> 2 min (mostly important to
  download larger logs)
* Template variables are passed to renderer using temporary file rather than
  command-line (working around limitations of sudo command line)



.. _changelog-0.10.0:

Verwalter 0.10.0
----------------

* Experimental webassembly scheduler support


.. _changelog-0.9.14:

Verwalter 0.9.14
----------------

* UI: fix chunk size in log tailer, mistakenly committed debugging version
* scheduler: if scheduler continue to fail for 5 min verwalter restarts on
  this node (this effectively elects a new leader)


.. _changelog-0.9.13:

Verwalter 0.9.13
----------------

* UI: add "Skip to End" button on log tail, skip by default on pressing "follow"


.. _changelog-0.9.12:

Verwalter 0.9.12
----------------

* Bugfix: fix crash on serving empty log
* Bugfix: JS error on the last step of api-frontend pipeline
* Log viewer leads to tail with correct offset


.. _changelog-0.9.11:

Verwalter 0.9.11
----------------

* Bugfix: Content-Range headers on logs were invalid
* Api-frontend: sorted server list
* Api-frontend: no "delete daemon" when update is active

.. _changelog-0.9.10:

Verwalter 0.9.10
----------------

* Add nicer log tailing UI and activate link in role log list
* Add some cantal metrics
* Bugfix: list of peers did not display correct timestamps

.. _changelog-0.9.9:

Verwalter 0.9.9
---------------

* Bugfix: external logs were not served properly
* Bugfix: when cantal fails for some time, verwalter could block


.. _changelog-0.9.8:

Verwalter 0.9.8
---------------

* Keeps few backups of old schedules
* Updates dependencies of frontend


.. _changelog-0.9.7:

Verwalter 0.9.7
---------------

* Bugfix: when request to cantal failed, verwalter would never reconnect


.. _changelog-0.9.6:

Verwalter 0.9.6
---------------

* Settings tweak: runtime load watchdog timeout is increased to 5 sec
* Bugfix: fix "rerender all roles" button (broken in 0.9.0)


.. _changelog-0.9.5:

Verwalter 0.9.5
---------------

* Bugfix: because we used unbuffered reading of runtime, it was too slow,
  effectively preventing scheduler to start on larger schedules
* Settings tweak: scheduler watchdog timeout is increased to 5 sec


.. _changelog-0.9.4:

Verwalter 0.9.4
---------------

* Bugfix: follower was unable to render templates (only leader)


.. _changelog-0.9.3:

Verwalter 0.9.3
---------------

* Peer info (known since, last ping) is now visible again (broken in 0.9.0)


.. _changelog-0.9.2:

Verwalter 0.9.2
---------------

* Fix bug in showing old schedule at ``/api/v1/schedule`` api
* Logs now served by newer library, so bigger subset of requests supported
  (last modified, no range, ...)

.. _changelog-0.9.1:

Verwalter 0.9.1
---------------

* Release packaging fixes and few dependencies upgraded


.. _changelog-0.9.0:

Verwalter 0.9.0
---------------

The mayor change in this version of scheduler that we migrated from rotor
network stack to tokio network stack. This is technically changes nothing
from user point of view. But we also decided to drop/fix rarely used functions
to make release more quick:

1. Dropped ``/api/v1/scheduler`` API, most useful info is now in
   ``/api/v1/status`` API
2. Some keys in status are changed
3. No metrics support any more, we'll reveal them in subsequent releases
   (we need more performant API in cantal for that)

Yes, we still use ``/v1`` and don't guarantee backwards compatibility
between 0.x releases. That would be a major pain.

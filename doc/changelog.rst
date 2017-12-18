Verwalter Changes by Version
============================


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

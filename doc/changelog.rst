Verwalter Changes by Version
============================


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

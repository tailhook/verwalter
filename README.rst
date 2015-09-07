=========
Verwalter
=========

:Status: Proof of Concept
:Documentation: http://verwalter.readthedocs.org

Verwalter manages local configuration data and signals processes for reload.
It's similar in spirit to confd_ or consul-template_ but has very different
feature set.

Verwalter also has optional resource management facility and may provide
service discovery too.


Features:

* Decentralized distribution of configuration
* Includes scriptable automated resource management (scripting via lua)
* Allows resource management even in minority partitions
* May provide discovery of only same-partition services in case of partitioning
* Liquid templates for configuration files
* Web interface to view current configuration state

Assumptions:

* Partitions happen
* Need some resource management in minority partition (even if it's essentially
  a "shutdown this service in minority partition", which can't be done if
  configuration is stored in zookeeper/consul/etcd)
* Need service discovery in minority partition, better if we could provide
  "only thease instances are available in current partition"

How it works:

* Collects metrics via cantal_
* Makes decisions in 10 second rounds
* Takes into account from 0.5 to 60 minutes of historical metrics
* Checks reachable nodes at each round
* Uses raft-like algorithm with weaker consistency guarantees


.. _cantal: http://cantal.readthedocs.org
.. _confd: https://github.com/kelseyhightower/confd
.. _consul-template: https://github.com/hashicorp/consul-template

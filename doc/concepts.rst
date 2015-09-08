========
Concepts
========

Verwalter is a tool that manages cluster of services.

Briefly verwalter provides the following:

* Cluster-wide management that is scriptable by lua_
* Limited service discovery

It builds on top of lithos_ (which is isolation, containarization and
supervising service) and cantal_ (which is sub-real-time monitoring and node
discovery service).

Verwalter is a framework for long-running services. It has abstractions to
configure running 10 instances of service X or use 7% of capacity for service
Y. And this resources will be consumed until configuration changed. This is
in contrast with Mesos_ or Yarn_ which has "start task A until it completes"
abstraction. (However, Verwalter can run and scale Mesos or Yarn cluster).


Components
==========

Let's look through each component of the system first, to understand what it
does before we can describe fully conceptual system.

Note the setup of the cluster is flat, you need all three components
``verwalter``, ``lithos`` and ``cantal`` on all nodes.

Lithos
------

Lithos_ is esentially a process supervisor. It's workflow is following:

1. Read configuration at ``/etc/lithos/sandboxes``
2. For each sandbox read configuration in ``/etc/lithos/processes``
3. Prepare the sandbox a/k/a linux container
4. Start process and keep restarting if that fails
5. Add/remove process if configuration changed

Lithos provides all necessary isolation for running processes (except it does
not handle network yet at the moment of writing), but it's super-simple
comparing to docker_ and mesos_ (i.e. mesos-slave) and even systemd_:

* Lithos reads configuration from files, no network calls needed (note the
  security impact)
* Lithos can restart itself in-place, keeping track of proccesses, so it's
  mostly crash-proof
* On ``SIGHUP`` signal for configuration change it just restarts itself

The **security model** of lithos_ is the ground for security of whole
verwalter-based cluster, so let's take a look:

* It's expected that *sandboxes* configs are predefined by administrators, and
  are not dynamically changed (either by verwalter or any other tool)
* Sandbox config limits folders, users, and few other system limits that
  application can't escape
* The command-line to run in sandbox is determined by configuration in image
  for that application

All this means that verwalter can only change the following things:

* Image (i.e. version of image) to run command from
* The name of the command to run from possible for that image
* Number of processes to run

I.e. whatever evil would be in verwalter's script it can't run arbitrary
command line on any host. So can't install rootkit, steal users' passwords and
do any other harm except taking down the cluster (which is an expected
permission for resource scheduler). This is in contrast to docker_/swarm
and mesos_ which allow to run anything.


Cantal
------

The cantal_ is a semi-real-time monitoring tool. It delivers statistics in
unusually short intervals and provides *node* discovery.

We use it:

* As a node discovery and availability monitoring
* For looking at current metrics of started application in nearly real-time
* As a liveness check for applications (mostly by looking at metrics)
* For collecting metrics from all nodes and aggregating
* Limited amount of historical data (~1 hour) is also used


Verwalter
---------

The verwalter is final piece of the puzzle to build fully working and
auto-rebalancing cluster.

In particular it does the following:

1. Establishes leader of the cluster (or a subcluster in case of split-brain)
2. Leader runs model of the cluster defined by sysadmin and augmented with lua
   scripts, to get number of processes run at each machine (and other
   important pieces of configuration).
3. Leader delivers configuration to every other node
4. At every node the configuration is rendered to local configuration files
   (most importantly ``/etc/lithos/processes``, but other types of
   configuration are supported too), and respective processes are notified.
5. All nodes display web frontend to review configuration. Frontend also has
   actionable buttons for common maintainance tasks like software upgrade or
   remove node from cluster

Unlike popular combinations of etcd_ + confd_, consul_ + consul-template_, or
mesos_ with whatever framework, verwalter can do scheduling decisions in
split-brain scenario even in minority partition. Verwalter is not a database so
having two leaders is not a problem when used wisely.

.. note:: Yes you can control how small cluster must be for cluster model to
   work and you can configure different reactions in majority and minority
   partition. I.e. doing any decisions on a single node isolated from 1000
   other nodes is useless. But switching off external `memcache` instance for
   the sake of running local one may be super-useful if you have a
   micro-service running on just two nodes.


The Missing Parts
-----------------

In current implementation the missing part is delivering files to node, in
particular:

1. Lithos configs ``/etc/lithos/master.yaml``, ``/etc/lithos/sandboxes``
2. Images of systems for lithos

We use ansible_ and good old rsync_ for these things for now


The Big Picture
===============

.. figure:: pic/boxes.svg
   :width: 300px
   :figwidth: 300px
   :align: right
   :alt: organization of proccesses on boxes

   All three processes [C]antal,
   [L]ithos and [V]erwalter on every machine

The cluster setup is simple. We have only one type of node and that node
runs three lightweight processes: lithos_, cantal_ and verwalter.

As outlined above cantal_ does node discovery by UDP. When the node first time
becomes up, it needs to join the cluster. Joining the cluster is done
by issuing a request::

    curl http://some.known.host:22682/add_host.json -d '{"addr": "1.2.3.4:22682"}'

.. warning:: This is not a stable API, so it may change at any time.

.. figure:: pic/cantal-gossip.svg
   :width: 300px
   :figwidth: 310px
   :align: left
   :alt: cantal gossip protocol

   Propagation of cluster join message

As the nodes are all equal you can issue a request to any node, or you can add
any existing node of a cluster to the new node, it doesn't matter. All the
info will quickly propagate to other nodes via gossip protocol.

As illustrated on the picture the discovery is random. But it tuned well to
efficiently cover whole network.

.. figure:: pic/cantal-init.svg
   :width: 300px
   :figwidth: 310px
   :align: right
   :alt: cantal supplies cluster information on verwalter's request

   Initial request of cluster info

When starting up, verwalter requests cluster information **from local cantal
instance**. The information consists of:

* list of peers in the cluster
* availability of the nodes (i.e. time of last successful ping)

Verwalter delegates all the work of joining cluster to cantal. As described
above, verwalter operates in one of the two modes: leader and follower. It
starts as follower and waits until it will be reached by leader. Leader in
turn discovers followers through cantal. I.e. it assumes that every cantal that
joins the cluster has verwalter instance.

While cantal is joining cluster and verwalter does it's own boostrapping and
possible leader election, the lithos continues to run. I.e. if there was any
configuration for lithos before reboot of the system or before you do any
maintainance of the verwalter/consul, the processes are started and supervised.
Any processes that crash are restarted and so on.

.. note:: In case you don't want for processes to start on boot, you may
   configure system to clean lithos configs on reboot (for example by putting
   them on ``tmpfs`` filesystem). This is occassionally useful, but we consider
   the default behaviour to start all processes that was previously run more
   useful in most cases.


.. _lithos: http://github.com/tailhook/lithos
.. _cantal: http://cantal.readthedocs.org
.. _lua: http://lua.org
.. _mesos: http://mesos.apache.org/
.. _yarn: http://hadoop.apache.org/docs/current/hadoop-yarn/hadoop-yarn-site/YARN.html
.. _docker: http://docker.com
.. _ansible: http://ansible.com
.. _rsync: https://en.wikipedia.org/wiki/Rsync
.. _systemd: http://www.freedesktop.org/wiki/Software/systemd/
.. _etcd: https://coreos.com/etcd/
.. _confd: http://www.confd.io/
.. _consul: https://www.consul.io/
.. _consul-template: https://github.com/hashicorp/consul-template

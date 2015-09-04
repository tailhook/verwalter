========
Concepts
========

Verwalter is a tool that manages cluster of services.

Briefly verwalter provides the following:

* Cluster-wide management that is scriptable by lua_
* Limited service discovery

It builds on top of lithos_ (which is isolation, containarization and
supervising service) and cantal_ (which is sub-real-time monitoring service).

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

Unlike popular combinations of etcd_ + confd_, consul_ + consul-template_, or
mesos_ with whatever framework, verwalter can do scheduling decisions in
split-brain scenario even in minority partition. Verwalter is not a database so
having two leaders is not a problem most of the time.

.. note:: Yes you can control how small cluster must be for cluster model to
   work and you can configure different reactions in majority and minority
   partition. I.e. doing any decisions on a single node isolated from 1000
   other nodes is useless. But switching off external `memcache` instance for
   the sake of running local one may be super-useful if you have a
   micro-service running on just two nodes.


The Missing Parts
-----------------

In current implementation the missing part is delivering file to node, in
particular:

1. Lithos configs ``/etc/lithos/master.yaml``, ``/etc/lithos/sandboxes``
2. Images of systems for lithos

We use ansible_ and good old rsync_ for these things for now


The Big Picture
===============


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

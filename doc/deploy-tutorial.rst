===================
Tutorial Deployment
===================


.. warning:: This is a work in progress tutorial for work in progress tools.
   It's not ready for use yet.


Brief
=====

This tutorial will guide you though deploying simple django_ application using
vagga_, lithos_, cantal_ and verwalter.


Tools
-----

We are trying to assume as little as possible about the reader knowledge, but
basic understanding of unix is definitely required. Here is the description
of tools that most readers would be intoduced here to:

vagga
  A tool for setting up development environments. For this tutorial, we will
  use it for building container images. Similar tools: vagrant_,
  docker-compose_, otto_, packer_ (in some sense).

lithos
  A container supervisor. This one starts containers in production environment.
  Unlike docker_ it doesn't have tools for building and fetching container
  images we will use vagga_ and rsync_ for that tasks. Similar tools: docker_,
  rocket_, systemd-nspawn_.

cantal
  A monitoring system, or a system collecting statistics. It's main
  distinction is that it is decentralized. It stores data in memory, and keeps
  only recent data. This makes it fast and highly-available. And this in turn
  allows to make orchestration decisions based on the metrics. Another feature
  is that it has built-in peer discovery. Similar tools: collectd_,
  prometheus_, graphite_.

verwalter
  A orchestration system. It's highly scriptable and decentralized. Meaning
  you can do orchestration tasks in split-brain scenario and it depends on you
  what specific things system can actually do. The tool also includes
  text templates for rendering configuration for any external system that is
  included in the cluster. Similar tools: mesos_, kubernetes_.

Any tool can potentially replaced by some other tool. Currently, the only hard
dependency is that you need cantal to run verwalter.

Anyway this combination provides good robustness, security and ease of use.
See :ref:`Concepts <concepts>` for more details about how these tools rely on each other
to provide mentioned features.





.. _django: https://www.djangoproject.com/
.. _vagga: http://github.com/tailhook/vagga
.. _lithos: http://github.com/tailhook/lithos
.. _cantal: http://github.com/tailhook/cantal
.. _vagrant: https://www.vagrantup.com/
.. _docker-compose: https://docs.docker.com/compose/
.. _docker: https://www.docker.com/
.. _packer: https://www.packer.io/intro/
.. _otto: https://www.ottoproject.io/
.. _rocket: https://github.com/coreos/rkt
.. _systemd-nspawn: https://www.freedesktop.org/software/systemd/man/systemd-nspawn.html
.. _collectd: https://collectd.org/
.. _graphite: http://graphite.wikidot.com/
.. _prometheus: https://prometheus.io/
.. _mesos: http://mesos.apache.org/
.. _kubernetes: http://kubernetes.io/
.. _rsync: https://rsync.samba.org/

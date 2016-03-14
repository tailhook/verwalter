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
See :ref:`Concepts <concepts>` for more details about how these tools rely on
each other to provide mentioned features.


Container
=========

Usually you start with a vagga container that works locally. There is a
tutorial_ for building a container for django application. We will skip this
part and assume you have a working container. Please, don't skip this part
even if you have development environment already set up (but not
containerized). It is important for the following reasons:

1. You need to know all dependencies and their versions, in may happen that
   you don't know exact list of system dependencies if you are using
   virtualenv for example.

2. Vagga_ makes everything readonly by default, so as lithos_. This serves
   as additional check of which filesystem paths are writable by the
   application (hopefully you don't have any).

3. We'll need the container for the next steps. We will base our deployment
   container on the development one (see below)

It's also good idea to make add a check of whether your application needs a
writable ``/tmp``. Just add a volume to your vagga container config:

.. code-block:: yaml

    containers:
      django:
        ...
        volumes:
          /tmp: !Empty

This makes ``/tmp`` read-only. So you can see errors when application tries
to write there and either fix the application (preferred in my opinion) or
provide valid ``/tmp`` mount in lithos configs later on.


.. _tutorial: http://vagga.readthedocs.org/en/latest/examples/tutorials/django.html

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

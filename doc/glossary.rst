========
Glossary
========


deployment id
  The unique identifier of the series of the actions that was run to apply
  certain config. Deployment id is local for single machine, but may span
  across roles. Single deployment id is used only once, so they refer to
  the time range when deployment started and finished. Multiple deployments
  can't be run on single machines simultaneously.

  Not all roles can be deployed with the single deployment id just the ones
  which need an update. Each role may execute commands only once during
  single deployment.

  There is no direct correspondence between config hash and deployment id.
  Single config may be deployed multiple times even on single machine.
  (each time when verwalter is restarted, each time when config changed and
  then rolled back again). But single deployment may deploy only single
  configuration. I.e. configuration can't change during deployment.

  And there is no direct match between application update and deployment id.
  The (rolling) application id usually involves multiple configuration updates.
  And each configuration update triggers one deployment on each machine.
  Also multiple rolling updates of different applications may take place at
  the same time. And all of them correspond to a single configuration change
  at any point in time.

=========
Rendering
=========

In verwalter "rendering" is a process of applying schedule to configure
specific application. It may consist of::

1. Substituting variables in textual templates
2. Running shell commands
3. Sending signals to other processes or different kind of IPC
4. *Possible, but discouraged:* calling HTTP APIs

Rendering for every role is deemed to be indepenedent of other roles. We also
encourage, but cannot enforce the following properties:

1. Atomic render of role (i.e. either it applied entirely, or not at all)
2. Full configuration check before switching


Input
=====

Input to the rendering process is a mapping of variables to values. For each
role we merge the following items from schedule:


    * ``vars``
    * ``roles[role_name]``
    * ``nodes[node_name]["vars"]``
    * ``nodes[node_name]["roles"][role_name]``

Where latter variables override former ones.

Nested mappings are merged up to two level's deep. I.e. if ``vars["common"]``
is a mapping each key of it will be updated by ``roles[x]["vars"]["common"]``
independently, but ``vars["common"]["info"]`` would be replaced
as a single atomic unit, regardless of whether it is an object or a string.

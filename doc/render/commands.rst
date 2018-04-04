Render Commands
===============


Condition
---------

Condition is a special command that executes other commands only if some
condition happens.

Example:

.. code-block: yaml

   templates:
     nginx: nginx.conf.trm
   commands:
   - !Copy
     src: "{{ templates.nginx }}"
     target: /etc/nginx/nginx.conf
   - !Condition
     dirs-changed: [/etc/nginx]
     commands:
     - !RootCommand [pkill, -HUP, nginx]

Conditions:

``dirs-changed``
    Calculates hash of all files in the directory recursively at the beginning
    of the **processing this .render.yaml** file. Then the hashsum is checked
    again when ``!Condition`` is encountered and if hashsum changed
    ``commands`` are executed, otherwise they are silently skipped.

Options:

``commands``
   List of commands to execute when condition is true. All the same commands
   suported except the ``!Condition`` itself.


CleanFiles
----------

Cleans files by pattern, keeping only ones listed.

Example:

.. code-block: yaml

   templates:
     list: keep_list.txt.trm
   commands:
   - !CleanFiles
     keep-list: "{{ list }}"
     pattern: /etc/nginx/sites-available/(*).conf


Options:

``pattern``
    Filename pattern to check. This supports basic **glob** syntax plus
    any part of path can be *captured* like in regular expression. This means
    that only parenthised part is matched against keep list, and only files
    that match glob are removed.

    Few pattern examples:

    * ``/dir/(*).conf`` deletes ``*.conf`` files, ``keep-list`` contains
      file names without extension
    * ``/dir/(*.conf)``, same but ``keep-list`` contains filenames with
      extension
    * ``/dir/(**/*.conf)``, deletes ``*.conf`` recursively, where keep list
      contains relative path (*without* ``./``)

``keep-list``
    Filename of the file which lists **names** which should be kept. Each
    line represents single name. The contents of each line matched against
    thing captured in ``pattern`` (see above). No comments or escaping
    is supported, empty lines are ignored.


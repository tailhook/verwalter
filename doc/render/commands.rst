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

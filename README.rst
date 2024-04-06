Simple Rust and Python based CMS
================================

Copyright (c) 2011-2024 Michael Buesch <m@bues.ch>


Installing
==========

Run the `build.sh` script to build the CMS system.

After building, run the `install-users.sh` script to create the user/group structure for CMS in the operating system.

After that, run the `install.sh` script.
It will install the CMS system into `/opt/cms/`.

Then create the database inside of `/opt/cms/etc/cms/db/`.
You may start with the example db:

.. code:: sh

    cp -r ./example/db/* /opt/cms/etc/cms/db/


Configuring Apache httpd
========================

Configure the CMS CGI binary as CGI `ScriptAlias`:

.. code::

    ScriptAlias /cms /opt/cms/libexec/cms-cgi/cms.cgi

    <Directory /opt/cms/libexec/cms-cgi>
        AllowOverride None
        Options +ExecCGI -MultiViews +SymLinksIfOwnerMatch -Indexes
        Require all granted
    </Directory>

    # Redirect all 404 to the CMS 404 handler (optional)
    ErrorDocument 404 /cms/__nopage/__nogroup.html

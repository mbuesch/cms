cms.py - simple WSGI/Python based CMS script
============================================

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

Install the Apache WSGI module. On Debian Linux, this is libapache2-mod-wsgi-py3.
Create a new config file /etc/apache2/conf.d/wsgi with content similar to
the following Debian based example:

.. code::

    # Adjust "user" and "group" to your system.
    WSGIDaemonProcess wsgi processes=10 threads=1 display-name=apache-wsgi user=www-data group=www-data python-path=/opt/cms/lib/python3/site-packages
    WSGIApplicationGroup %{GLOBAL}
    WSGIPythonOptimize 1
    # /cms is the base URL path to the CMS.
    # /opt/cms/share/cms-wsgi is where the index.wsgi script lives.
    # /opt/cms/etc/cms/db is where the database directory lives.
    # /var/www/html is the path to the static files.
    # Adjust these paths to your setup.
    WSGIScriptAlias                    /cms        /opt/cms/share/cms-wsgi/index.wsgi
    SetEnv cms.domain                  example.com
    SetEnv cms.db                      /opt/cms/etc/cms/db
    SetEnv cms.wwwBase                 /var/www/html
    SetEnv cms.maxPostContentLength    1048576
    SetEnv cms.debug                   1
    <Directory /opt/cms/share/cms-wsgi>
        WSGIProcessGroup wsgi
        AllowOverride None
        Options -ExecCGI -MultiViews +SymLinksIfOwnerMatch -Indexes
        Require all granted
    </Directory>
    # Redirect all 404 to the CMS 404 handler (optional)
    ErrorDocument 404 /cms/__nopage/__nogroup.html

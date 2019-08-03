cms.py - simple WSGI/Python based CMS script
============================================

Copyright (c) 2011-2019 Michael Buesch <m@bues.ch>


Installing
==========

Just clone the cms.git repository to some directory where apache has access to.
In the configuration example below the directory will be ``/var/cms``

Then create the database directory named ``db`` inside of the cloned directory.
You may start with the example db:

``cp -r /var/cms/example/db /var/cms/``


Configuring Apache httpd
========================

Install the Apache WSGI module. On Debian Linux, this is libapache2-mod-wsgi-py3.
Create a new config file /etc/apache2/conf.d/wsgi with content similar to
the following Debian based example:

.. code::

    # Adjust "user" and "group" to your system.
    WSGIDaemonProcess wsgi processes=10 threads=1 display-name=apache-wsgi user=www-data group=www-data python-path=/var/cms
    WSGIPythonOptimize 1
    # /cms is the base URL path to the CMS.
    # /var/cms is where index.wsgi and the db directory live.
    # /var/www is the path to the static files.
    # Adjust these paths to your setup.
    WSGIScriptAlias                    /cms        /var/cms/index.wsgi
    SetEnv cms.domain                  example.com
    SetEnv cms.cmsBase                 /var/cms
    SetEnv cms.wwwBase                 /var/www
    SetEnv cms.maxPostContentLength    1048576
    SetEnv cms.debug                   1
    <Directory /var/cms>
        WSGIProcessGroup wsgi
        AllowOverride None
        Options -ExecCGI -MultiViews +SymLinksIfOwnerMatch -Indexes
        Require all granted
    </Directory>
    # Redirect all 404 to the CMS 404 handler (optional)
    ErrorDocument 404 /cms/__nopage/__nogroup.html

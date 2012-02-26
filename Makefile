DEFAULT_DESTDIR	:= /var/cms
DESTDIR		:= $(DEFAULT_DESTDIR)
DEFAULT_OWNER	:= www-data
OWNER		:= $(DEFAULT_OWNER)
DEFAULT_GROUP	:= www-data
GROUP		:= $(DEFAULT_GROUP)

INSTALL		:= install
CP		:= cp
CHOWN		:= chown


all: help

install: help

help:
	@echo "Use  make install-cms  to install the cms.py script"
	@echo "Use  make install-db  to install the example database"
	@echo
	@echo "Use  make install-world  to install all of the above"
	@echo
	@echo "To adjust the install target path, set the DESTDIR variable."
	@echo "DESTDIR defaults to $(DEFAULT_DESTDIR)"
	@echo "To adjust the permissions of the target directories and files,"
	@echo "set the OWNER and GROUP variables."
	@echo "OWNER defaults to $(DEFAULT_OWNER)"
	@echo "GROUP defaults to $(DEFAULT_GROUP)"

$(DESTDIR):
	$(INSTALL) -d -o $(OWNER) -g $(GROUP) -m 755 $(DESTDIR)

install-cms: $(DESTDIR)
	$(INSTALL) -o $(OWNER) -g $(GROUP) -m 644 cms.py $(DESTDIR)/
	$(INSTALL) -o $(OWNER) -g $(GROUP) -m 644 index.wsgi $(DESTDIR)/

install-db: $(DESTDIR)
	$(CP) -a example/db $(DESTDIR)/
	$(CHOWN) -R $(OWNER):$(GROUP) $(DESTDIR)/db

install-world: install-cms install-db

.PHONY: all help install install-cms install-db install-world

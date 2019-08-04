#!/usr/bin/python 3
# -*- coding: utf-8 -*-
#
#   CMS WSGI wrapper
#
#   Copyright (C) 2011-2019 Michael Buesch <m@bues.ch>
#
#   This program is free software: you can redistribute it and/or modify
#   it under the terms of the GNU General Public License as published by
#   the Free Software Foundation, either version 2 of the License, or
#   (at your option) any later version.
#
#   This program is distributed in the hope that it will be useful,
#   but WITHOUT ANY WARRANTY; without even the implied warranty of
#   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#   GNU General Public License for more details.
#
#   You should have received a copy of the GNU General Public License
#   along with this program.  If not, see <http://www.gnu.org/licenses/>.

try:
	from cms_cython.wsgi_app import *
except ImportError as e:
	try:
		from cms.wsgi_app import *
	except ImportError as e:
		raise Exception("Failed to import cms.wsgi_app. "\
			"Wrong python-path in WSGIDaemonProcess?:\n" + str(e))

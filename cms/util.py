# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
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

#from cms.cython_support cimport * #@cy

from cms.exception import *

import os
import cgi
from stat import S_ISDIR
from io import BytesIO
from datetime import datetime

__all__ = [
	"BytesIO",
	"datetime",
	"UPPERCASE",
	"LOWERCASE",
	"NUMBERS",
	"isiterable",
	"findNot",
	"findAny",
	"htmlEscape",
	"stringBool",
	"mkpath",
	"f_exists",
	"f_exists_nonempty",
	"f_read",
	"f_read_int",
	"f_mtime",
	"f_mtime_nofail",
	"f_subdirList",
]

UPPERCASE = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
LOWERCASE = 'abcdefghijklmnopqrstuvwxyz'
NUMBERS   = '0123456789'

# Check if an object is iterable.
def isiterable(obj):
	try:
		iter(obj)
		return True
	except TypeError:
		pass # obj is not an iterable.
	except Exception:
		raise CMSException(500, "isiterable: Unexpected exception.")
	return False

# Find the index in 'string' that is _not_ in 'template'.
# Start search at 'idx'.
# Returns -1 on failure to find.
def findNot(string, template, idx=0):
	while idx < len(string):
		if string[idx] not in template:
			return idx
		idx += 1
	return -1

# Find the index in 'string' that matches _any_ character in 'template'.
# Start search at 'idx'.
# Returns -1 on failure to find.
def findAny(string, template, idx=0):
	while idx < len(string):
		if string[idx] in template:
			return idx
		idx += 1
	return -1

def htmlEscape(string):
	return cgi.escape(string, True)

def stringBool(string, default=False):
	s = string.lower()
	if s in ("true", "yes", "on", "1"):
		return True
	if s in ("false", "no", "off", "0"):
		return False
	try:
		return bool(int(s, 10))
	except ValueError:
		return default

# Create a path string from path element strings.
def mkpath(*path_elements):
	# Do not use os.path.join, because it discards elements, if
	# one element begins with a separator (= is absolute).
	return os.path.sep.join(path_elements)

def f_exists(*path_elements):
	try:
		os.stat(mkpath(*path_elements))
	except OSError:
		return False
	return True

def f_exists_nonempty(*path_elements):
	if f_exists(*path_elements):
		return bool(f_read(*path_elements).strip())
	return False

def f_read(*path_elements):
	try:
		with open(mkpath(*path_elements), "rb") as fd:
			return fd.read().decode("UTF-8")
	except IOError:
		return ""
	except UnicodeError:
		raise CMSException(500, "Unicode decode error")

def f_read_int(*path_elements):
	data = f_read(*path_elements)
	try:
		return int(data.strip(), 10)
	except ValueError:
		return None

def f_mtime(*path_elements):
	try:
		return datetime.utcfromtimestamp(os.stat(mkpath(*path_elements)).st_mtime)
	except OSError:
		raise CMSException(404)

def f_mtime_nofail(*path_elements):
	try:
		return f_mtime(*path_elements)
	except CMSException:
		return datetime.utcnow()

def f_subdirList(*path_elements):
	def dirfilter(dentry):
		if dentry.startswith("."):
			return False # Omit ".", ".." and hidden entries
		if dentry.startswith("__"):
			return False # Omit system folders/files.
		try:
			if not S_ISDIR(os.stat(mkpath(path, dentry)).st_mode):
				return False
		except OSError:
			return False
		return True
	path = mkpath(*path_elements)
	try:
		return [ dentry for dentry in os.listdir(path) \
			 if dirfilter(dentry) ]
	except OSError:
		return []

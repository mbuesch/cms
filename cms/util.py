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

import cgi
import datetime
import io
import os
import stat

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
	"fs",
]

BytesIO = io.BytesIO				#+cdef-public-object
datetime = datetime.datetime			#+cdef-public-object

UPPERCASE = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'	#+cdef-public-str
LOWERCASE = 'abcdefghijklmnopqrstuvwxyz'	#+cdef-public-str
NUMBERS   = '0123456789'			#+cdef-public-str

# Check if an object is iterable.
def isiterable(obj): #@nocy
#cdef ExBool_t isiterable(object obj) except ExBool_val: #@cy
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
def findNot(string, template, idx): #@nocy
#cdef _Bool findNot(str string, str template, int64_t idx): #@cy
#@cy	cdef int64_t slen

	if idx >= 0:
		slen = len(string)
		while idx < slen:
			if string[idx] not in template:
				return idx
			idx += 1
	return -1

# Find the index in 'string' that matches _any_ character in 'template'.
# Start search at 'idx'.
# Returns -1 on failure to find.
def findAny(string, template, idx): #@nocy
#cdef _Bool findAny(str string, str template, int64_t idx): #@cy
#@cy	cdef int64_t slen

	if idx >= 0:
		slen = len(string)
		while idx < slen:
			if string[idx] in template:
				return idx
			idx += 1
	return -1

def htmlEscape(string): #@nocy
#cdef str htmlEscape(str string): #@cy
	return cgi.escape(string, True)

def stringBool(string, default): #@nocy
#cdef _Bool stringBool(str string, _Bool default): #@cy
	s = string.lower()
	if s in ("true", "yes", "on", "1"):
		return True
	if s in ("false", "no", "off", "0"):
		return False
	try:
		return bool(int(s, 10))
	except ValueError:
		return default

class FSHelpers(object): #+cdef
	# Create a path string from path element strings.
	def mkpath(self, *path_elements):
		# Do not use os.path.join, because it discards elements, if
		# one element begins with a separator (= is absolute).
		return os.path.sep.join(path_elements)

	def exists(self, *path_elements):
		try:
			os.stat(self.mkpath(*path_elements))
		except OSError:
			return False
		return True

	def exists_nonempty(self, *path_elements):
		if self.exists(*path_elements):
			return bool(self.read(*path_elements).strip())
		return False

	def read(self, *path_elements):
		try:
			with open(self.mkpath(*path_elements).encode("UTF-8", "strict"), "rb") as fd:
				return fd.read().decode("UTF-8", "strict")
		except IOError:
			return ""
		except UnicodeError:
			raise CMSException(500, "Unicode decode error")

	def read_int(self, *path_elements):
		data = self.read(*path_elements)
		try:
			return int(data.strip(), 10)
		except ValueError:
			return None

	def mtime(self, *path_elements):
		try:
			return datetime.utcfromtimestamp(os.stat(self.mkpath(*path_elements)).st_mtime)
		except OSError:
			raise CMSException(404)

	def mtime_nofail(self, *path_elements):
		try:
			return self.mtime(*path_elements)
		except CMSException:
			return datetime.utcnow()

	def subdirList(self, *path_elements):
		def dirfilter(dentry):
			if dentry.startswith("."):
				return False # Omit ".", ".." and hidden entries
			if dentry.startswith("__"):
				return False # Omit system folders/files.
			try:
				if not stat.S_ISDIR(os.stat(self.mkpath(path, dentry)).st_mode):
					return False
			except OSError:
				return False
			return True
		path = self.mkpath(*path_elements)
		try:
			return [ dentry for dentry in os.listdir(path) \
				 if dirfilter(dentry) ]
		except OSError:
			return []

fs = FSHelpers() #+cdef-FSHelpers

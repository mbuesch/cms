# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2011-2023 Michael Buesch <m@bues.ch>
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

import html
import datetime as datetime_mod
import io
import os
import stat

BytesIO = io.BytesIO				#+cdef-public-object
datetime = datetime_mod.datetime		#+cdef-public-object
dt_timezone = datetime_mod.timezone		#+cdef-public-object

__all__ = [
	"BytesIO",
	"datetime",
	"dt_timezone",
	"UPPERCASE",
	"LOWERCASE",
	"NUMBERS",
	"isiterable",
	"findNot",
	"findAny",
	"htmlEscape",
	"stringBool",
	"StrCArray",
	"str2carray",
	"carray2str",
	"fs",
]

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
	return html.escape(string, True)

def stringBool(string, default): #@nocy
#cdef _Bool stringBool(str string, _Bool default): #@cy
#@cy	cdef str s

	s = string.lower()
	if s in ("true", "yes", "on", "1"):
		return True
	if s in ("false", "no", "off", "0"):
		return False
	try:
		return bool(int(s))
	except ValueError:
		return default

class StrCArray(object):				#@nocy
	__slots__ = ( "_string", )			#@nocy

def str2carray(carray, string, arrayLen):		#@nocy
	if arrayLen > 0:				#@nocy
		carray._string = string[:arrayLen-1]	#@nocy

def carray2str(carray, arrayLen):			#@nocy
	if arrayLen > 0:				#@nocy
		return carray._string			#@nocy
	return ""					#@nocy

class FSHelpers(object): #+cdef
	def __init__(self):
		self.__pathSep = os.path.sep
		self.__os_stat = os.stat
		self.__os_listdir = os.listdir
		self.__stat_S_ISDIR = stat.S_ISDIR

	def __mkpath(self, path_elements): #@nocy
#@cy	cdef str __mkpath(self, tuple path_elements):
		# Do not use os.path.join, because it discards elements, if
		# one element begins with a separator (= is absolute).
		return self.__pathSep.join(path_elements)

	# Create a path string from path element strings.
	def mkpath(self, *path_elements):
		return self.__mkpath(path_elements)

	def __exists(self, path_elements): #@nocy
#@cy	cdef _Bool __exists(self, tuple path_elements):
#@cy		cdef str path

		try:
			path = self.__mkpath(path_elements)
			self.__os_stat(path.encode("UTF-8", "strict"))
		except OSError:
			return False
		except UnicodeError:
			raise CMSException(500, "Unicode decode error")
		return True

	def exists(self, *path_elements):
		return self.__exists(path_elements)

	def __exists_nonempty(self, path_elements): #@nocy
#@cy	cdef _Bool __exists_nonempty(self, tuple path_elements):
		if self.__exists(path_elements):
			return bool(self.__read(path_elements).strip())
		return False

	def exists_nonempty(self, *path_elements):
		return self.__exists_nonempty(path_elements)

	def __read(self, path_elements): #@nocy
#@cy	cdef str __read(self, tuple path_elements):
#@cy		cdef str path
#@cy		cdef bytes data

		try:
			path = self.__mkpath(path_elements)
			with open(path.encode("UTF-8", "strict"), "rb") as fd:
				data = fd.read()
				return data.decode("UTF-8", "strict")
		except IOError:
			return ""
		except UnicodeError:
			raise CMSException(500, "Unicode decode error")

	def read(self, *path_elements):
		return self.__read(path_elements)

	def __read_int(self, path_elements): #@nocy
#@cy	cdef object __read_int(self, tuple path_elements):
#@cy		cdef str data

		data = self.__read(path_elements)
		try:
			return int(data.strip())
		except ValueError:
			return None

	def read_int(self, *path_elements):
		return self.__read_int(path_elements)

	def __mtime(self, path_elements): #@nocy
#@cy	cdef object __mtime(self, tuple path_elements):
		try:
			path = self.__mkpath(path_elements).encode("UTF-8", "strict")
			return datetime.utcfromtimestamp(self.__os_stat(path).st_mtime)
		except OSError:
			raise CMSException(404)
		except UnicodeError:
			raise CMSException(500, "Unicode decode error")

	def mtime(self, *path_elements):
		return self.__mtime(path_elements)

	def __mtime_nofail(self, path_elements): #@nocy
#@cy	cdef object __mtime_nofail(self, tuple path_elements):
		try:
			return self.__mtime(path_elements)
		except CMSException:
			return datetime.now(dt_timezone.utc)

	def mtime_nofail(self, *path_elements):
		return self.__mtime_nofail(path_elements)

	def __mode(self, path_elements): #@nocy
#@cy	cdef object __mode(self, tuple path_elements):
		try:
			path = self.__mkpath(path_elements).encode("UTF-8", "strict")
			return self.__os_stat(path).st_mode
		except OSError:
			raise CMSException(404)
		except UnicodeError:
			raise CMSException(500, "Unicode decode error")

	def mode(self, *path_elements):
		return self.__mode(path_elements)

	def __subdirList(self, path_elements): #@nocy
#@cy	cdef list __subdirList(self, tuple path_elements):
#@cy		cdef str path
#@cy		cdef bytes bdentry
#@cy		cdef bytes bp
#@cy		cdef str dentry
#@cy		cdef list dirListing
#@cy		cdef list ret

		try:
			ret = []
			path = self.__mkpath(path_elements)
			dirListing = self.__os_listdir(path.encode("UTF-8", "strict"))
			S_ISDIR = self.__stat_S_ISDIR
			stat = self.__os_stat
			for bdentry in dirListing:
				dentry = bdentry.decode("UTF-8", "strict")
				if dentry.startswith("."):
					continue # Omit ".", ".." and hidden entries
				if dentry.startswith("__"):
					continue # Omit system folders/files.
				try:
					bp = self.__mkpath((path, dentry)).encode("UTF-8", "strict")
					if not S_ISDIR(stat(bp).st_mode):
						continue
				except OSError:
					continue
				except UnicodeError:
					raise CMSException(500, "Unicode decode error")
				ret.append(dentry)
			return ret
		except OSError:
			return []
		except UnicodeError:
			raise CMSException(500, "Unicode decode error")

	def subdirList(self, *path_elements):
		return self.__subdirList(path_elements)

fs = FSHelpers() #+cdef-FSHelpers

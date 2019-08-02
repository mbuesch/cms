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

from cms.exception import *
from cms.util import *

import re
import os

__all__ = [
	"CMSPageIdent",
]

class CMSPageIdent(object):
	# Page identifier.

	__slots__ = (
		"__elements",
		"__allValidated",
	)

	__pageFileName_re	= re.compile(
		r'^(.*)((?:\.html?)|(?:\.py)|(?:\.php))$', re.DOTALL)
	__indexPages		= {"", "index"}

	# Parse a page identifier from a string.
	@classmethod
	def parse(cls, path, maxPathLen=512, maxIdentDepth=32):
		if len(path) > maxPathLen:
			raise CMSException(400, "Invalid URL")

		pageIdent = cls()

		# Strip whitespace and slashes
		path = path.strip(' \t/')

		# Remove page file extensions like .html and such.
		m = cls.__pageFileName_re.match(path)
		if m:
			path = m.group(1)

		# Use the ident elements, if this is not the root page.
		if path not in cls.__indexPages:
			pageIdent.extend(path.split("/"))

		if len(pageIdent.__elements) > maxIdentDepth:
			raise CMSException(400, "Invalid URL")

		return pageIdent

	__pathSep = os.path.sep
	__validPathChars = LOWERCASE + UPPERCASE + NUMBERS + "-_."

	# Validate a path component. Avoid any directory change.
	# Raises CMSException on failure.
	@classmethod
	def validateSafePathComponent(cls, pcomp):
		if pcomp.startswith('.'):
			# No ".", ".." and hidden files.
			raise CMSException(404, "Invalid page path")
		if [ c for c in pcomp if c not in cls.__validPathChars ]:
			raise CMSException(404, "Invalid page path")
		return pcomp

	# Validate a path. Avoid going back in the hierarchy (. and ..)
	# Raises CMSException on failure.
	@classmethod
	def validateSafePath(cls, path):
		for pcomp in path.split(cls.__pathSep):
			cls.validateSafePathComponent(pcomp)
		return path

	# Validate a page name.
	# Raises CMSException on failure.
	# If allowSysNames is True, system names starting with "__" are allowed.
	@classmethod
	def validateName(cls, name, allowSysNames=False):
		if name.startswith("__") and not allowSysNames:
			# Page names with __ are system folders.
			raise CMSException(404, "Invalid page name")
		return cls.validateSafePathComponent(name)

	# Initialize this page identifier.
	def __init__(self, initialElements=None):
		self.__elements = []
		self.extend(initialElements)
		self.__allValidated = False

	# Add a list of path elements to this identifier.
	def extend(self, other):
		if other is not None:
			self.__allValidated = False

			if isinstance(other, self.__class__):
				self.__elements.extend(other.__elements)
			elif isiterable(other):
				self.__elements.extend(other)
			else:
				raise CMSException(500, "Invalid 'other' in CMSPageIdent.extend()")
		return self

	# Add a list of path elements to this identifier.
	def __iadd__(self, other):
		return self.extend(other)

	# Create a new page identifier from 'self' and add 'other'.
	def __add__(self, other):
		return self.__class__(self).extend(other)

	# Get the number of path components in this path identifier.
	def __len__(self):
		return len(self.__elements)

	# Validate all page identifier name components.
	# (Do not allow system name components)
	def __validateAll(self):
		if not self.__allValidated:
			for pcomp in self.__elements:
				self.validateName(pcomp)
			# Remember that we validated.
			# (This flag must be reset to false, if components are added.)
			self.__allValidated = True

	# Get one page identifier component by index.
	def get(self, index, default=None, allowSysNames=False):
		try:
			return self.validateName(self.__elements[index],
						 allowSysNames)
		except IndexError:
			return default

	# Get the page identifier as URL.
	def getUrl(self, protocol=None, domain=None,
		   urlBase=None, pageSuffix=".html"):
		self.__validateAll()
		url = []
		if protocol:
			url.append(protocol + ":/")
		if domain:
			url.append(domain)
		if urlBase:
			url.append(urlBase.strip("/"))
		localPath = [elem for elem in self.__elements if elem]
		url.extend(localPath)
		if not protocol and not domain:
			url.insert(0, "")
		urlStr = "/".join(url)
		if localPath and pageSuffix:
			urlStr += pageSuffix
		return urlStr

	# Get the page identifier as filesystem path.
	def getFilesystemPath(self, rstrip=0):
		self.__validateAll()
		if self.__elements:
			if rstrip:
				pcomps = self.__elements[ : 0 - rstrip]
				if pcomps:
					return mkpath(*pcomps)
				return ""
			return mkpath(*(self.__elements))
		return ""

	# Test if this identifier starts with the same elements
	# as another one.
	def startswith(self, other):
		return other is not None and\
		       len(self.__elements) >= len(other.__elements) and\
		       self.__elements[ : len(other.__elements)] == other.__elements

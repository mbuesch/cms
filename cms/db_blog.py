# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2023 Michael Buesch <m@bues.ch>
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
from cms.pageident import *
from cms.util import * #+cimport

__all__ = [
	"BlogDatabase",
]

class BlogDatabase:
	validate = CMSPageIdent.validateName

	def __init__(self, basePath):
		self.basePath = basePath

	def __mkBlogPath(self, blogName, year=None, month=None, day=None, entry=None):
		path = self.basePath
		for component in (blogName, year, month, day, entry):
			if component is None:
				break
			component = self.validate(component)
			path = fs.mkpath(path, component)
		return path

	def beginSession(self):
		pass

	def getBlogsList(self):
		res = []
		for blogName in fs.subdirList(self.basePath):
			if blogName.startswith("__"):
				continue
			blogPath = fs.mkpath(self.basePath, blogName)
			res.append( (blogName,) )
		return res

	def getBlogEntryList(self, blogName, year=None, month=None, day=None):
		path = self.__mkBlogPath(blogName, year, month, day)
		res = []
		for entry in fs.subdirList(path):
			if entry.startswith("__"):
				continue
			entryPath = fs.mkpath(path, entry)
			mode = fs.mode(entryPath)
			if mode & 0o004 == 0: # Not read-other
				continue
			mtime = fs.mtime(entryPath)
			res.append( (entry, mtime) )
		return res

	def getBlog(self, blogName, year, month, day, entry):
		path = self.__mkBlogPath(blogName, year, month, day, entry)
		data = fs.read(path)
		return (data,)

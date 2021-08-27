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
from cms.pageident import *
from cms.util import * #+cimport

import re
import sys
import importlib.machinery
import functools

__all__ = [
	"CMSDatabase",
]

class CMSDatabase(object):
	validate = CMSPageIdent.validateName

	def __init__(self, basePath):
		self.pageBase = fs.mkpath(basePath, "pages")
		self.macroBase = fs.mkpath(basePath, "macros")
		self.stringBase = fs.mkpath(basePath, "strings")

	def beginSession(self):
		# Clear all lru_cache.
		self.getMacro.cache_clear()

	def __redirect(self, redirectString):
		raise CMSException301(redirectString)

	def __getPageTitle(self, pagePath):
		title = fs.read(pagePath, "title").strip()
		if not title:
			title = fs.read(pagePath, "nav_label").strip()
		return title

	def getNavStop(self, pageIdent):
		path = fs.mkpath(self.pageBase, pageIdent.getFilesystemPath())
		return bool(fs.read_int(path, "nav_stop"))

	def getHeader(self, pageIdent):
		path = fs.mkpath(self.pageBase, pageIdent.getFilesystemPath())
		return fs.read(path, "header.html")

	def getPage(self, pageIdent):
		path = fs.mkpath(self.pageBase, pageIdent.getFilesystemPath())
		redirect = fs.read(path, "redirect").strip()
		if redirect:
			return self.__redirect(redirect)
		title = self.__getPageTitle(path)
		data = fs.read(path, "content.html")
		stamp = fs.mtime_nofail(path, "content.html")
		return (title, data, stamp)

	def getPageTitle(self, pageIdent):
		path = fs.mkpath(self.pageBase, pageIdent.getFilesystemPath())
		return self.__getPageTitle(path)

	def getPageStamp(self, pageIdent):
		path = fs.mkpath(self.pageBase, pageIdent.getFilesystemPath())
		return fs.mtime_nofail(path, "content.html")

	# Get a list of sub-pages.
	# Returns list of (pagename, navlabel, prio)
	def getSubPages(self, pageIdent, sortByPrio = True):
		res = []
		gpath = fs.mkpath(self.pageBase, pageIdent.getFilesystemPath())
		for pagename in fs.subdirList(gpath):
			path = fs.mkpath(gpath, pagename)
			if fs.exists(path, "hidden") or \
			   fs.exists_nonempty(path, "redirect"):
				continue
			navlabel = fs.read(path, "nav_label").strip()
			prio = fs.read_int(path, "priority")
			if prio is None:
				prio = 500
			res.append( (pagename, navlabel, prio) )
		if sortByPrio:
			res.sort(key = lambda e: "%010d_%s" % (e[2], e[1]))
		return res

	# Get the contents of a @MACRO().
	# This method is cached for this db session.
	# So one page using one macro multiple times is cheap.
	@functools.lru_cache(maxsize=2**6)
	def getMacro(self, macroname, pageIdent=None):
		data = None
		macroname = self.validate(macroname)
		if pageIdent:
			rstrip = 0
			while not data:
				path = pageIdent.getFilesystemPath(rstrip)
				if not path:
					break
				data = fs.read(self.pageBase,
					      path,
					      "__macros",
					      macroname)
				rstrip += 1
		if not data:
			data = fs.read(self.pageBase,
				      "__macros",
				      macroname)
		if not data:
			data = fs.read(self.macroBase, macroname)
		return '\n'.join( l for l in data.splitlines() if l )

	def getString(self, name, default=None):
		name = self.validate(name)
		string = fs.read(self.stringBase, name).strip()
		if string:
			return string
		return default or ""

	def getPostHandler(self, pageIdent):
		path = fs.mkpath(self.pageBase, pageIdent.getFilesystemPath())
		handlerModFile = fs.mkpath(path, "post.py")

		if not fs.exists(handlerModFile):
			return None

		# Add the path to sys.path, so that post.py can easily import
		# more files from its directory.
		if path not in sys.path:
			sys.path.insert(0, path)

		try:
			loader = importlib.machinery.SourceFileLoader(
				re.sub(r"[^A-Za-z]", "_", handlerModFile),
				handlerModFile)
			mod = loader.load_module()
		except OSError:
			return None

		mod.CMSException = CMSException
		mod.CMSPostException = CMSPostException

		return getattr(mod, "post", None)

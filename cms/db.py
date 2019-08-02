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
from cms.util import *

import re
import sys
import importlib.machinery

__all__ = [
	"CMSDatabase",
]

class CMSDatabase(object):
	validate = CMSPageIdent.validateName

	def __init__(self, basePath):
		self.pageBase = mkpath(basePath, "pages")
		self.macroBase = mkpath(basePath, "macros")
		self.stringBase = mkpath(basePath, "strings")

	def __redirect(self, redirectString):
		raise CMSException301(redirectString)

	def __getPageTitle(self, pagePath):
		title = f_read(pagePath, "title").strip()
		if not title:
			title = f_read(pagePath, "nav_label").strip()
		return title

	def getNavStop(self, pageIdent):
		path = mkpath(self.pageBase, pageIdent.getFilesystemPath())
		return bool(f_read_int(path, "nav_stop"))

	def getHeader(self, pageIdent):
		path = mkpath(self.pageBase, pageIdent.getFilesystemPath())
		return f_read(path, "header.html")

	def getPage(self, pageIdent):
		path = mkpath(self.pageBase, pageIdent.getFilesystemPath())
		redirect = f_read(path, "redirect").strip()
		if redirect:
			return self.__redirect(redirect)
		title = self.__getPageTitle(path)
		data = f_read(path, "content.html")
		stamp = f_mtime_nofail(path, "content.html")
		return (title, data, stamp)

	def getPageTitle(self, pageIdent):
		path = mkpath(self.pageBase, pageIdent.getFilesystemPath())
		return self.__getPageTitle(path)

	# Get a list of sub-pages.
	# Returns list of (pagename, navlabel, prio)
	def getSubPages(self, pageIdent, sortByPrio = True):
		res = []
		gpath = mkpath(self.pageBase, pageIdent.getFilesystemPath())
		for pagename in f_subdirList(gpath):
			path = mkpath(gpath, pagename)
			if f_exists(path, "hidden") or \
			   f_exists_nonempty(path, "redirect"):
				continue
			navlabel = f_read(path, "nav_label").strip()
			prio = f_read_int(path, "priority")
			if prio is None:
				prio = 500
			res.append( (pagename, navlabel, prio) )
		if sortByPrio:
			res.sort(key = lambda e: "%010d_%s" % (e[2], e[1]))
		return res

	def getMacro(self, macroname, pageIdent = None):
		data = None
		macroname = self.validate(macroname)
		if pageIdent:
			rstrip = 0
			while not data:
				path = pageIdent.getFilesystemPath(rstrip)
				if not path:
					break
				data = f_read(self.pageBase,
					      path,
					      "__macros",
					      macroname)
				rstrip += 1
		if not data:
			data = f_read(self.pageBase,
				      "__macros",
				      macroname)
		if not data:
			data = f_read(self.macroBase, macroname)
		return '\n'.join( l for l in data.splitlines() if l )

	def getString(self, name, default=None):
		name = self.validate(name)
		string = f_read(self.stringBase, name).strip()
		if string:
			return string
		return default or ""

	def getPostHandler(self, pageIdent):
		path = mkpath(self.pageBase, pageIdent.getFilesystemPath())
		handlerModFile = mkpath(path, "post.py")

		if not f_exists(handlerModFile):
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

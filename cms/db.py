# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2011-2024 Michael Buesch <m@bues.ch>
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
from cms.util import fs, datetime #+cimport
from cms.db_socket import *

import re
import sys
import importlib.machinery

__all__ = [
	"CMSDatabase",
]

class CMSDatabase(object):
	validate = CMSPageIdent.validateName

	def __init__(self, basePath):
		self.pageBase = fs.mkpath(basePath, "pages")
		try:
			self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
			self.sock.connect("/run/cms-fsd.sock")
		except Exception:
			raise CMSException(500, "cms-fsd communication error")

	def __communicate(self, msg):
		try:
			self.sock.sendall(msg.pack())
			return recv_message(self.sock)
		except Exception:
			raise CMSException(500, "cms-fsd communication error")

	@staticmethod
	def __encode(s):
		try:
			if s is not None:
				return s.encode("UTF-8", "strict")
		except UnicodeError:
			pass
		return b""

	@staticmethod
	def __decode(b):
		try:
			if b is not None:
				return b.decode("UTF-8", "strict")
		except UnicodeError:
			pass
		return ""

	def __redirect(self, redirectString):
		raise CMSException301(redirectString)

	def getNavStop(self, pageIdent):
		reply = self.__communicate(MsgGetPage(
			path=pageIdent.getFilesystemPath(),
			get_title=False,
			get_data=False,
			get_stamp=False,
			get_prio=False,
			get_redirect=False,
			get_nav_stop=True,
			get_nav_label=False,
		))
		nav_stop = bool(reply.nav_stop)
		return nav_stop

	def getHeaders(self, pageIdent):
		reply = self.__communicate(MsgGetHeaders(
			path=pageIdent.getFilesystemPath(),
		))
		data = self.__decode(reply.data)
		return data

	def getPage(self, pageIdent):
		reply = self.__communicate(MsgGetPage(
			path=pageIdent.getFilesystemPath(),
			get_title=True,
			get_data=True,
			get_stamp=True,
			get_prio=False,
			get_redirect=True,
			get_nav_stop=False,
			get_nav_label=False,
		))
		redirect = self.__decode(reply.redirect).strip()
		if redirect:
			return self.__redirect(redirect)
		title = self.__decode(reply.title)
		data = self.__decode(reply.data)
		stamp = datetime.utcfromtimestamp(reply.stamp or 0)
		return (title, data, stamp)

	def getPageTitle(self, pageIdent):
		reply = self.__communicate(MsgGetPage(
			path=pageIdent.getFilesystemPath(),
			get_title=True,
			get_data=False,
			get_stamp=False,
			get_prio=False,
			get_redirect=False,
			get_nav_stop=False,
			get_nav_label=False,
		))
		title = self.__decode(reply.title)
		return title

	def getPageStamp(self, pageIdent):
		reply = self.__communicate(MsgGetPage(
			path=pageIdent.getFilesystemPath(),
			get_title=False,
			get_data=False,
			get_stamp=True,
			get_prio=False,
			get_redirect=False,
			get_nav_stop=False,
			get_nav_label=False,
		))
		stamp = datetime.utcfromtimestamp(reply.stamp or 0)
		return stamp

	# Get a list of sub-pages.
	# Returns list of (pagename, navlabel, prio)
	def getSubPages(self, pageIdent, sortByPrio=True):
		reply = self.__communicate(MsgGetSubPages(
			path=pageIdent.getFilesystemPath(),
		))
		res = []
		for i in range(len(reply.pages)):
			pagename = self.__decode(reply.pages[i])
			navlabel = self.__decode(reply.nav_labels[i]).strip()
			prio = reply.prios[i]
			res.append( (pagename, navlabel, prio) )
		if sortByPrio:
			res.sort(key = lambda e: "%010d_%s" % (e[2], e[1]))
		return res

	# Get the contents of a @MACRO().
	def getMacro(self, macroname, pageIdent=None):
		reply = self.__communicate(MsgGetMacro(
			parent=pageIdent.getFilesystemPath() if pageIdent is not None else "",
			name=macroname,
		))
		data = self.__decode(reply.data)
		return '\n'.join( l for l in data.splitlines() if l )

	def getString(self, name, default=None):
		reply = self.__communicate(MsgGetString(
			name=name,
		))
		string = self.__decode(reply.data).strip()
		if string:
			return string
		return default or ""

	# formFields: CMSFormData: Basically a dict
	# query: CMSQuery: Basically a dict
	def runPostHandler(self, pageIdent, formFields, query):
		path = fs.mkpath(self.pageBase, pageIdent.getFilesystemPath())
		handlerModFile = fs.mkpath(path, "post.py")

		if not fs.exists(handlerModFile):
			raise CMSException(405)

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
			raise CMSException(405)

		mod.CMSException = CMSException
		mod.CMSPostException = CMSPostException

		postHandler = getattr(mod, "post", None)
		if postHandler is None:
			raise CMSException(405)

		return postHandler(formFields, query, b"", "", "")

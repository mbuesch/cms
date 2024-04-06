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

__all__ = [
	"CMSException",
	"CMSException301",
]

class CMSException(Exception):
	__stats = {
		301 : "Moved Permanently",
		400 : "Bad Request",
		404 : "Not Found",
		405 : "Method Not Allowed",
		409 : "Conflict",
		500 : "Internal Server Error",
	}

	def __init__(self, httpStatusCode=500, message=""):
		try:
			httpStatus = self.__stats[httpStatusCode]
		except KeyError:
			httpStatusCode = 500
			httpStatus = self.__stats[httpStatusCode]
		self.httpStatusCode = httpStatusCode
		self.httpStatus = "%d %s" % (httpStatusCode, httpStatus)
		self.message = message

	def getHttpHeaders(self, resolveCallback):
		return ()

	def getHtmlHeader(self, db):
		return ""

	def getHtmlBody(self, db):
		return db.getString('http-error-page',
				    '<p style="font-size: large;">%s</p>' %\
				    self.httpStatus)

class CMSException301(CMSException):
	# "Moved Permanently" exception

	def __init__(self, newUrl):
		CMSException.__init__(self, 301, newUrl)

	def url(self):
		return self.message

	def getHttpHeaders(self, resolveCallback):
		return ( ('Location', resolveCallback(self.url())), )

	def getHtmlHeader(self, db):
		return '<meta http-equiv="refresh" content="0; URL=%s" />' %\
			self.url()

	def getHtmlBody(self, db):
		return '<p style="font-size: large;">' \
			'Moved permanently to ' \
			'<a href="%s">%s</a>' \
			'</p>' %\
			(self.url(), self.url())

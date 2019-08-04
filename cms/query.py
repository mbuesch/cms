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
from cms.util import * #+cimport

__all__ = [
	"CMSQuery",
]

class CMSQuery(object):
	__slots__ = (
		"__queryDict",
	)

	def __init__(self, queryDict):
		self.__queryDict = {
			name : values[-1]
			for name, values in queryDict.items()
		}

	def items(self):
		return self.__queryDict.items()

	def get(self, name, default=""):
		return self.__queryDict.get(name, default)

	def getInt(self, name, default=0):
		try:
			return int(self.__queryDict.get(name, default))
		except (ValueError) as e:
			return default

	def getBool(self, name, default=False):
		return stringBool(self.__queryDict.get(name, str(default)), default)

	def __hash__(self):
		return hash(frozenset(self.__queryDict.items()))

	def __eq__(self, other):
		return (isinstance(other, self.__class__) and
			self.__queryDict == other.__queryDict)

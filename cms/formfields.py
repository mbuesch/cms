# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2011-2022 Michael Buesch <m@bues.ch>
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

import cms.multipart as multipart

__all__ = [
	"CMSFormFields",
]

class CMSFormFields(object):
	"""Form field parser.
	"""

	__slots__ = (
		"__forms",
	)

	defaultCharset		= LOWERCASE + UPPERCASE + NUMBERS + "-_. \t"
	defaultCharsetBool	= LOWERCASE + UPPERCASE + NUMBERS + " \t"
	defaultCharsetInt	= NUMBERS + "xXabcdefABCDEF- \t"

	def __init__(self, body, bodyType):
		try:
			forms, files = multipart.parse_form_data(
				environ={
					"wsgi.input"		: BytesIO(body),
					"CONTENT_LENGTH"	: str(len(body)),
					"CONTENT_TYPE"		: bodyType,
					"REQUEST_METHOD"	: "POST",
				}
			)
			self.__forms = forms
		except Exception as e:
			raise CMSException(400, "Cannot parse form data.")

	def getStr(self, name, default="", maxlen=32, charset=defaultCharset):
		"""Get a form field.
		Returns a str.
		"""
		field = self.__forms.get(name, default)
		if field is None:
			return None
		if maxlen is not None and len(field) > maxlen:
			raise CMSException(400, "%s is too long." % name)
		if charset is not None and [ c for c in field if c not in charset ]:
			raise CMSException(400, "Invalid character in %s" % name)
		return field

	def getBool(self, name, default=False, maxlen=32, charset=defaultCharsetBool):
		"""Get a form field.
		Returns a bool.
		"""
		field = self.getStr(name, None, maxlen, charset)
		if field is None:
			return default
		return stringBool(field, default)

	def getInt(self, name, default=0, maxlen=32, charset=defaultCharsetInt):
		"""Get a form field.
		Returns an int.
		"""
		field = self.getStr(name, None, maxlen, charset)
		if field is None:
			return default
		try:
			field = field.lower().strip()
			if field.startswith("0x"):
				return int(field[2:], 16)
			return int(field)
		except ValueError:
			return default

# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2011-2021 Michael Buesch <m@bues.ch>
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
import random

cround = round #@nocy
#from libc.math cimport round as cround #@cy

__all__ = [
	"CMSStatementResolver",
]

MACRO_STACK_SIZE	= 64 #@nocy
MACRO_STACK_NAME_SIZE	= 32 #@nocy

# Call stack element
class _StackElem(object):		#@nocy
	__slots__ = (			#@nocy
		"name",			#@nocy
		"lineno",		#@nocy
	)				#@nocy
	def __init__(self):		#@nocy
		self.name = StrCArray()	#@nocy

def stackElem(lineno, name): #@nocy
#cdef inline _StackElem stackElem(int64_t lineno, str name): #@cy
#@cy	cdef _StackElem se

	se = _StackElem() #@nocy
	se.lineno = lineno
	str2carray(se.name, name, MACRO_STACK_NAME_SIZE)
	return se

class _IndexRef(object): #+cdef
	# Index references
	def __init__(self, charOffset):
		self.charOffset = charOffset

class _Anchor(object): #+cdef
	# HTML anchor
	def __init__(self, name, text,
		     indent=-1, noIndex=False):
		self.name = name
		self.text = text
		self.indent = indent
		self.noIndex = noIndex

	def makeUrl(self, resolver): #@nocy
#@cy	cdef str makeUrl(self, CMSStatementResolver resolver):
		return "%s#%s" % (resolver.expandVariable("CMS_PAGEIDENT"),
				  self.name)

class _ArgParserRet(object):	#@nocy
	__slots__ = (		#@nocy
		"cons",		#@nocy
		"arguments",	#@nocy
	)			#@nocy

class _ResolverRet(object):	#@nocy
	__slots__ = (		#@nocy
		"cons",		#@nocy
		"data",		#@nocy
	)			#@nocy

def resolverRet(cons, data): #@nocy
#cdef inline _ResolverRet resolverRet(int64_t cons, str data): #@cy
#@cy	cdef _ResolverRet r

	r = _ResolverRet()
	r.cons = cons
	r.data = data
	return r

class CMSStatementResolver(object): #+cdef
	__genericVars = {
		"BR"		: "<br />",
		"DOMAIN"	: lambda self, n: self.cms.domain,
		"CMS_BASE"	: lambda self, n: self.cms.urlBase,
		"IMAGES_DIR"	: lambda self, n: self.cms.imagesDir,
		"THUMBS_DIR"	: lambda self, n: self.cms.urlBase + "/__thumbs",
		"DEBUG"		: lambda self, n: "1" if self.cms.debug else "",
		"__DUMPVARS__"	: lambda self, n: self.__dumpVars(),
	}

	def __init__(self, cms):
		self.__handlers = self._handlers
		self.__escapedChars = self._escapedChars
		# Valid characters for variable names (without the leading $)
		self.VARNAME_CHARS = UPPERCASE + '_'

		self.cms = cms
		self.__reset()

	def __reset(self, variables={}, pageIdent=None):
		self.variables = variables.copy()
		self.variables.update(self.__genericVars)
		self.pageIdent = pageIdent
		self.charCount = 0
		self.indexRefs = []
		self.anchors = []

		self.__callStack = [ None ] * MACRO_STACK_SIZE #@nocy
		self.__macroArgs = [ None ] * MACRO_STACK_SIZE
		self.__callStack[0] = stackElem(1, "content.html")
		self.__callStackLen = 1

	def __stmtError(self, msg):
#@cy		cdef _StackElem se

		pfx = ""
		if self.cms.debug:
			se = self.__callStack[self.__callStackLen - 1]
			pfx = "%s:%d: " % (carray2str(se.name, MACRO_STACK_NAME_SIZE),
					   se.lineno)
		raise CMSException(500, pfx + msg)

	def expandVariable(self, name): #@nocy
#@cy	cdef str expandVariable(self, str name):
		try:
			value = self.variables[name]
			if callable(value):
				value = value(self, name)
			if value is None:
				raise KeyError
			return str(value)
		except (KeyError, TypeError) as e:
			return ""

	def __dumpVars(self, force=False):
		if not force and not self.cms.debug:
			return ""
		ret = []
		for name in sorted(self.variables.keys()):
			if name == "__DUMPVARS__":
				value = "-- variable dump --"
			else:
				value = self.expandVariable(name)
			sep = "\t" * (3 - len(name) // 8)
			ret.append("%s%s=> %s" % (name, sep, value))
		return "\n".join(ret)

	_escapedChars = '\\,@$()'

	def escape(self, data): #@nocy
#@cy	cpdef str escape(self, str data):
#@cy		cdef str c

		for c in self.__escapedChars:
			data = data.replace(c, '\\' + c)
		return data

	def unescape(self, data): #@nocy
#@cy	cpdef str unescape(self, str data):
#@cy		cdef str c

		for c in self.__escapedChars:
			data = data.replace('\\' + c, c)
		return data

	# Parse statement arguments.
	def __parseArguments(self, d, dOffs, strip): #@nocy
#@cy	cdef _ArgParserRet __parseArguments(self, str d, int64_t dOffs, _Bool strip):
#@cy		cdef _ResolverRet r
#@cy		cdef _ArgParserRet ret
#@cy		cdef int64_t dEnd
#@cy		cdef int64_t i
#@cy		cdef str data

		ret = _ArgParserRet()
		ret.cons = 0
		ret.arguments = []

		i = dOffs
		dEnd = len(d)
		while i < dEnd:
			r = self.__expandRecStmts(d, i, ',)')
			data = r.data
			i += r.cons

			ret.cons += r.cons
			ret.arguments.append(data.strip() if strip else data)

			if i <= dOffs or i - 1 >= dEnd or d[i - 1] == ')':
				break
		return ret

	def __stmt_if(self, d, dOffs):
		"""
		Evaluate a CONDITION and return THEN or ELSE based on the CONDITION.
		If ELSE is not specified, then this statement uses an empty string instead of ELSE.

		Statement: $(if CONDITION, THEN, ELSE)
		Statement: $(if CONDITION, THEN)

		Returns: THEN if CONDITION is not empty after stripping whitespace.
		Returns: ELSE otherwise.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args
#@cy		cdef str condition
#@cy		cdef str b_then
#@cy		cdef str b_else

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2 and len(args) != 3:
			self.__stmtError("IF: invalid number of arguments (%d)" %\
					 len(args))
		condition, b_then = args[0], args[1]
		b_else = args[2] if len(args) == 3 else ""
		result = b_then if condition.strip() else b_else
		return resolverRet(cons, result)

	def __do_compare(self, d, dOffs, invert): #@nocy
#@cy	cdef _ResolverRet __do_compare(self, str d, int64_t dOffs, _Bool invert):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args
#@cy		cdef str arg
#@cy		cdef str firstArg
#@cy		cdef _Bool result

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if len(args) < 2:
			self.__stmtError("EQ/NE: invalid args")
		result = True
		firstArg = args[0]
		for arg in args[1:]:
			result = result and arg == firstArg
		if invert:
			result = not result
		return resolverRet(cons, ("1" if result else ""))

	def __stmt_eq(self, d, dOffs):
		"""
		Compares two or more strings for equality.

		Statement: $(eq A, B, ...)

		Returns: 1, if all stripped arguments are equal.
		Returns: An empty string otherwise.
		"""
		return self.__do_compare(d, dOffs, False)

	def __stmt_ne(self, d, dOffs):
		"""
		Compares two or more strings for inequality.

		Statement: $(ne A, B, ...)

		Returns: 1, if not all stripped arguments are equal.
		Returns: An empty string otherwise.
		"""
		return self.__do_compare(d, dOffs, True)

	def __stmt_and(self, d, dOffs):
		"""
		Compares all arguments with logical AND operation.

		Statement: $(and A, B, ...)

		Returns: The first stripped argument (A), if all stripped arguments are non-empty strings.
		Returns: An empty string otherwise.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if len(args) < 2:
			self.__stmtError("AND: invalid args")
		return resolverRet(cons, (args[0] if all(args) else ""))

	def __stmt_or(self, d, dOffs):
		"""
		Compares all arguments with logical OR operation.

		Statement: $(or A, B, ...)

		Returns: The first stripped non-empty argument.
		Returns: An empty string, if there is no non-empty argument.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if len(args) < 2:
			self.__stmtError("OR: invalid args")
		nonempty = [ a for a in args if a ]
		return resolverRet(cons, (nonempty[0] if nonempty else ""))

	def __stmt_not(self, d, dOffs):
		"""
		Logically invert the boolean argument.

		Statement: $(not A)

		Returns: 1, if the stripped argument A is an empty string.
		Returns: An empty string otherwise.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if len(args) != 1:
			self.__stmtError("NOT: invalid args")
		return resolverRet(cons, ("" if args[0] else "1"))

	def __stmt_assert(self, d, dOffs):
		"""
		Debug assertion.
		Aborts the program, if any argument is an empty string.

		Statement: $(assert A, ...)

		Raises: A 500-assertion-failed exception, if any argument is empty after stripping.
		Returns: An empty string, otherwise.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if len(args) < 1:
			self.__stmtError("ASSERT: missing argument")
		if not all(args):
			self.__stmtError("ASSERT: failed")
		return resolverRet(cons, "")

	def __stmt_strip(self, d, dOffs):
		"""
		Strip whitespace at the start and at the end of all arguments.
		Concatenate all arguments.

		Statement: $(strip A, ...)

		Returns: All arguments stripped and concatenated.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		return resolverRet(cons, "".join(args))

	def __stmt_item(self, d, dOffs):
		"""
		Select an item from a list.
		Splits the STRING argument into tokens and return the N'th token.
		The token SEPARATOR defaults to whitespace.

		Statement: $(item STRING, N)
		Statement: $(item STRING, N, SEPARATOR)

		Returns: The N'th token.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in (2, 3):
			self.__stmtError("ITEM: invalid args")
		string, n, sep = args[0], args[1], args[2].strip() if len(args) == 3 else ""
		tokens = string.split(sep) if sep else string.split()
		try:
			token = tokens[int(n)]
		except ValueError:
			self.__stmtError("ITEM: N is not an integer")
		except IndexError:
			token = ""
		return resolverRet(cons, token)

	def __stmt_contains(self, d, dOffs):
		"""
		Check if a list contains an item.
		HAYSTACK is a list separated by SEPARATOR.
		SEPARATOR defaults to whitespace.

		Statement: $(contains HAYSTACK, NEEDLE)
		Statement: $(contains HAYSTACK, NEEDLE, SEPARATOR)

		Returns: NEEDLE, if HAYSTACK contains the stripped NEEDLE.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in (2, 3):
			self.__stmtError("CONTAINS: invalid args")
		haystack, needle, sep = args[0], args[1].strip(), args[2] if len(args) == 3 else ""
		tokens = haystack.split(sep) if sep else haystack.split()
		return resolverRet(cons, needle if needle in tokens else "")

	def __stmt_substr(self, d, dOffs):
		"""
		Cut a sub string out of the STRING argument.
		START is the first character index of the sub string.
		END is the last character index of the sub string plus 1.
		END defaults to START + 1.

		Statement: $(substr STRING, START)
		Statement: $(substr STRING, START, END)

		Returns: The sub string of STRING starting at START index up to (but not including) END index.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in (2, 3):
			self.__stmtError("SUBSTR: invalid args")
		string, start, end = args[0], args[1], args[2] if len(args) == 3 else ""
		try:
			if end.strip():
				substr = string[int(start) : int(end)]
			else:
				substr = string[int(start)]
		except ValueError:
			self.__stmtError("SUBSTR: START or END is not an integer")
		except IndexError:
			substr = ""
		return resolverRet(cons, substr)

	def __stmt_sanitize(self, d, dOffs):
		"""
		Sanitize a string.
		Concatenates all arguments with an underscore as separator.
		Replaces all non-alphanumeric characters by an underscore. Forces lower-case.

		Statement: $(sanitize STRING, ...)

		Returns: The sanitized string.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) < 1:
			self.__stmtError("SANITIZE: invalid args")
		string = "_".join(args)
		validChars = LOWERCASE + NUMBERS
		string = string.lower()
		string = "".join( c if c in validChars else '_' for c in string )
		string = re.sub(r'_+', '_', string).strip('_')
		return resolverRet(cons, string)

	def __stmt_fileExists(self, d, dOffs):
		"""
		Checks if a file exists relative to the wwwPath base.

		Statement: $(file_exists RELATIVE_PATH)
		Statement: $(file_exists RELATIVE_PATH, DOES_NOT_EXIST)

		Returns: The path, if the file exists.
		Returns: An empty string, if the file does not exist and DOES_NOT_EXIST is not specified.
		Returns: The DOES_NOT_EXIST argument, if the file does not exist.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in (1, 2):
			self.__stmtError("FILE_EXISTS: invalid args")
		relpath, enoent = args[0], args[1] if len(args) == 2 else ""
		try:
			exists = fs.exists(self.cms.wwwPath,
					   CMSPageIdent.validateSafePath(relpath))
		except (CMSException) as e:
			exists = False
		return resolverRet(cons, (relpath if exists else enoent))

	def __stmt_fileModDateTime(self, d, dOffs):
		"""
		Get the file modification time of the file at RELATIVE_PATH.
		RELATIVE_PATH is relative to wwwPath.
		FORMAT_STRING is an optional strftime format string.

		Statement: $(file_mdatet RELATIVE_PATH)
		Statement: $(file_mdatet RELATIVE_PATH, DOES_NOT_EXIST, FORMAT_STRING)

		Returns: The file modification time, if the file exists.
		Returns: An empty string, if the file does not exist and DOES_NOT_EXIST is not specified.
		Returns: The DOES_NOT_EXIST argument, if the file does not exist.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in (1, 2, 3):
			self.__stmtError("FILE_MDATET: invalid args")
		relpath, enoent, fmtstr =\
			args[0],\
			args[1] if len(args) >= 2 else "",\
			args[2] if len(args) >= 3 else "%d %B %Y %H:%M (UTC)"
		try:
			stamp = fs.mtime(self.cms.wwwPath,
					 CMSPageIdent.validateSafePath(relpath))
		except (CMSException) as e:
			return resolverRet(cons, enoent)
		return resolverRet(cons, stamp.strftime(fmtstr.strip()))

	def __stmt_index(self, d, dOffs):
		"""
		Generate the site index.

		Statement: $(index)

		Returns: The site index.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 1 or args[0]:
			self.__stmtError("INDEX: invalid args")
		self.indexRefs.append(_IndexRef(self.charCount))
		return resolverRet(cons, "")

	def __stmt_anchor(self, d, dOffs):
		"""
		Set an new site index anchor.
		NAME is the html-id of the new anchor.
		TEXT is the html-text of the new anchor.

		Statement: $(anchor NAME, TEXT)
		Statement: $(anchor NAME, TEXT, INDENT_LEVEL)
		Statement: $(anchor NAME, TEXT, INDENT_LEVEL, NO_INDEX)

		Returns: The site index anchor HTML code.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args
#@cy		cdef _Anchor anchor

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in (2, 3, 4):
			self.__stmtError("ANCHOR: invalid args")
		name, text = args[0:2]
		indent, noIndex = -1, False
		if len(args) >= 3:
			indentStr = args[2].strip()
			if indentStr:
				try:
					indent = int(indentStr)
				except ValueError:
					self.__stmtError("ANCHOR: indent level "
							 "is not an integer")
		if len(args) >= 4:
			noIndex = bool(args[3].strip())
		name, text = name.strip(), text.strip()
		anchor = _Anchor(name, text, indent, noIndex)
		# Cache anchor for index creation
		self.anchors.append(anchor)
		# Create the anchor HTML
		return resolverRet(cons, '<a id="%s" href="%s">%s</a>' %\
					 (name, anchor.makeUrl(self), text))

	def __stmt_pagelist(self, d, dOffs):
		"""
		Get the navigation elements html code of all sub-page names in the page.

		Statement: $(pagelist BASEPAGE)

		Returns: The navigation elements html code.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 1:
			self.__stmtError("PAGELIST: no base page argument")
		try:
			basePageIdent = CMSPageIdent.parse(args[0])
		except CMSException as e:
			self.__stmtError("PAGELIST: invalid base page name")
		html = []
		self.cms._genNavElem(html, basePageIdent, None, 1)
		return resolverRet(cons, '\n'.join(html))

	def __stmt_random(self, d, dOffs):
		"""
		Generate a random number.
		BEGIN defaults to 0.
		END defaults to 65535.

		Statement: $(random)
		Statement: $(random BEGIN)
		Statement: $(random BEGIN, END)

		Returns: A random integer in the range from BEGIN to END (including both end points).
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if len(args) not in (0, 1, 2):
			self.__stmtError("RANDOM: invalid args")
		begin, end = 0, 65535
		try:
			if len(args) >= 2 and args[1]:
				end = int(args[1])
			if len(args) >= 1 and args[0]:
				begin = int(args[0])
			rnd = random.randint(begin, end)
		except ValueError as e:
			self.__stmtError("RANDOM: invalid range")
		return resolverRet(cons, '%d' % rnd)

	def __stmt_randitem(self, d, dOffs):
		"""
		Select a random item.

		Statement: $(randitem ITEM0, ...)

		Returns: One random item of its arguments.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) < 1:
			self.__stmtError("RANDITEM: too few args")
		return resolverRet(cons, random.choice(args))

	def __do_arith(self, oper, args): #@nocy
#@cy	cdef str __do_arith(self, object oper, list args):
#@cy		cdef float res
#@cy		cdef int64_t rounded
#@cy		cdef float a
#@cy		cdef float b

		try:
			a = float(args[0])
		except ValueError as e:
			a = 0.0
		try:
			b = float(args[1])
		except ValueError as e:
			b = 0.0
		res = oper(a, b)
		rounded = int(cround(res))
		return ("%f" % res) if (abs(res - rounded) >= 0.000001)\
		       else str(rounded)

	def __stmt_add(self, d, dOffs):
		"""
		Add two numbers (integer or float).
		Returns the result as an integer, if it is representable as an integer.
		Otherwise returns the result as a floating point number.

		Statement: $(add A, B)

		Returns: The result of A + B
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("ADD: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a + b, args))

	def __stmt_sub(self, d, dOffs):
		"""
		Subtract two numbers (integer or float).
		Returns the result as an integer, if it is representable as an integer.
		Otherwise returns the result as a floating point number.

		Statement: $(sub A, B)

		Returns: The result of A - B
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("SUB: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a - b, args))

	def __stmt_mul(self, d, dOffs):
		"""
		Multiply two numbers (integer or float).
		Returns the result as an integer, if it is representable as an integer.
		Otherwise returns the result as a floating point number.

		Statement: $(mul A, B)

		Returns: The result of A * B
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("MUL: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a * b, args))

	def __stmt_div(self, d, dOffs):
		"""
		Divide two numbers (integer or float).
		Returns the result as an integer, if it is representable as an integer.
		Otherwise returns the result as a floating point number.

		Statement: $(div A, B)

		Returns: The result of A / B
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("DIV: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a / b, args))

	def __stmt_mod(self, d, dOffs):
		"""
		Divide two numbers (integer or float) and get the remainder.
		Returns the result as an integer, if it is representable as an integer.
		Otherwise returns the result as a floating point number.

		Statement: $(mod A, B)

		Returns: The result of remainder(A / B)
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("MOD: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a % b, args))

	def __stmt_round(self, d, dOffs):
		"""
		Round a floating point number to the next integer.
		If NDIGITS is specified, then round to this number of decimal digits.

		Statement: $(round A)
		Statement: $(round A, NDIGITS)

		Returns: Argument A rounded.
		"""
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in (1, 2):
			self.__stmtError("ROUND: invalid args")
		try:
			a = float(args[0])
		except ValueError as e:
			a = 0.0
		try:
			if len(args) == 1:
				res = str(int(cround(a)))
			else:
				try:
					n = int(args[1])
				except ValueError as e:
					n = 0
				n = max(min(n, 64), 0)
				res = ("%." + str(n) + "f") % int(round(a, n))
		except (ValueError, TypeError) as e:
			self.__stmtError("ROUND: invalid value")
		return resolverRet(cons, res)

	# statement handlers
	_handlers = {
		# conditional / string compare / boolean
		"$(if"		: __stmt_if,
		"$(eq"		: __stmt_eq,
		"$(ne"		: __stmt_ne,
		"$(and"		: __stmt_and,
		"$(or"		: __stmt_or,
		"$(not"		: __stmt_not,

		# debugging
		"$(assert"	: __stmt_assert,

		# string processing
		"$(strip"	: __stmt_strip,
		"$(item"	: __stmt_item,
		"$(contains"	: __stmt_contains,
		"$(substr"	: __stmt_substr,
		"$(sanitize"	: __stmt_sanitize,

		# filesystem access
		"$(file_exists"	: __stmt_fileExists,
		"$(file_mdatet"	: __stmt_fileModDateTime,

		# page index / page info
		"$(index"	: __stmt_index,
		"$(anchor"	: __stmt_anchor,
		"$(pagelist"	: __stmt_pagelist,

		# random numbers
		"$(random"	: __stmt_random,
		"$(randitem"	: __stmt_randitem,

		# arithmetic
		"$(add"		: __stmt_add,
		"$(sub"		: __stmt_sub,
		"$(mul"		: __stmt_mul,
		"$(div"		: __stmt_div,
		"$(mod"		: __stmt_mod,
		"$(round"	: __stmt_round,
	}

	def __doMacro(self, macroname, d, dOffs): #@nocy
#@cy	cdef _ResolverRet __doMacro(self, str macroname, str d, int64_t dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef _ResolverRet mRet
#@cy		cdef str macrodata
#@cy		cdef uint16_t callStackIdx

		if len(macroname) > MACRO_STACK_NAME_SIZE - 1:
			raise CMSException(500, "Macro name '%s' is too long." % macroname)

		if self.__callStackLen >= MACRO_STACK_SIZE:
			raise CMSException(500, "Exceed macro call stack depth")
		a = self.__parseArguments(d, dOffs, True)

		# Fetch the macro data from the database
		try:
			macrodata = self.cms.db.getMacro(macroname[1:],
							 self.pageIdent)
		except (CMSException) as e:
			macrodata = ""
			if e.httpStatusCode == 404:
				# Name validation failed.
				raise CMSException(500,
					"Macro name '%s' contains "
					"invalid characters" % macroname)
		if not macrodata:
			return resolverRet(a.cons, "")  # Macro does not exist.

		# Resolve statements and recursive macro calls
		callStackIdx = self.__callStackLen
		self.__callStack[callStackIdx] = stackElem(1, macroname)
		self.__macroArgs[callStackIdx] = a.arguments
		self.__callStackLen = callStackIdx + 1
		mRet = self.__expandRecStmts(macrodata, 0, "")
		macrodata = mRet.data
		self.__callStackLen -= 1

		return resolverRet(a.cons, macrodata)

	# Recursively expand statements and macro calls.
	def __expandRecStmts(self, d, dOffs, stopchars): #@nocy
#@cy	cdef _ResolverRet __expandRecStmts(self, str d, int64_t dOffs, str stopchars):
#@cy		cdef int64_t i
#@cy		cdef int64_t end
#@cy		cdef int64_t cons
#@cy		cdef int64_t dEnd
#@cy		cdef int64_t strip_nl
#@cy		cdef str stmtName
#@cy		cdef list ret
#@cy		cdef _ResolverRet macroRet
#@cy		cdef _ResolverRet handlerRet
#@cy		cdef _ResolverRet retObj
#@cy		cdef Py_UCS4 c
#@cy		cdef str res
#@cy		cdef uint16_t callStackIdx
#@cy		cdef uint16_t argIdx
#@cy		cdef list macroArgs
#@cy		cdef str macroArg
#@cy		cdef str macroName

		callStackIdx = self.__callStackLen - 1
		macroArgs = self.__macroArgs[callStackIdx]

		ret = []
		dEnd = len(d)
		i = dOffs
		while i < dEnd:
			c = d[i]
			res = None
			cons = 1
			if c == '\\':
				# Escaped characters
				# Keep escapes. They are removed later.
				if i + 1 < dEnd and\
				   d[i + 1] in self.__escapedChars:
					res = d[i:i+2]
					i += 1
			elif c == '\n':
				self.__callStack[callStackIdx].lineno += 1
			elif c == '<' and d.startswith('<!---', i):
				# Comment
				end = d.find('--->', i)
				if end > i:
					strip_nl = 0
					# If comment is on a line of its own,
					# remove the line.
					if (i == 0 or d[i - 1] == '\n') and\
					   (end + 4 < dEnd and d[end + 4] == '\n'):
						strip_nl = 1
					cons, res = end - i + 4 + strip_nl, ""
			elif c in stopchars:
				# Stop character
				i += 1
				break
			elif c == '@':
				# Macro call
				end = d.find('(', i)
				if end > i:
					macroRet = self.__doMacro(d[i:end], d, end+1)
					cons, res = macroRet.cons, macroRet.data
					i = end + 1
			elif (c == '$' and
			      i + 1 < dEnd and d[i + 1] in "0123456789"):
				# Macro argument.
				end = findNot(d, "0123456789", i + 1)
				if macroArgs is not None and end > i + 1:
					argIdx = int(d[i+1:end]) # this int conversion can't fail.
					if argIdx == 0:
						macroName = carray2str(self.__callStack[callStackIdx].name,
								       MACRO_STACK_NAME_SIZE)
						cons, res = end - i, macroName
					elif argIdx > 0 and argIdx <= len(macroArgs):
						macroArg = macroArgs[argIdx - 1] #+suffix-u
						cons, res = end - i, macroArg
					else:
						cons, res = end - i, ""
			elif c == '$' and i + 1 < dEnd and d[i + 1] == '(':
				# Statement
				end = findAny(d, ' )', i)
				if end > i:
					stmtName = d[i:end]
					if stmtName in self.__handlers:
						h = self.__handlers[stmtName]
						i = end + 1 if d[end] == ' ' else end
					else:
						h = lambda _a, _b, _c: resolverRet(cons, res) # nop
				handlerRet = h(self, d, i)
				cons, res = handlerRet.cons, handlerRet.data
			elif c == '$':
				# Variable
				end = findNot(d, self.VARNAME_CHARS, i + 1)
				if end > i + 1:
					res = self.expandVariable(d[i+1:end])
					cons = end - i
			if res is None:
				ret.append(c)
				self.charCount += 1
			else:
				ret.append(res)
				self.charCount += len(res)
			i += cons
		if i >= dEnd and stopchars and d[-1] not in stopchars:
			self.__stmtError("Unterminated statement")

		retObj = resolverRet(i - dOffs, "".join(ret))
		self.charCount -= len(retObj.data)
		return retObj

	# Create an index
	def __createIndex(self, anchors):
#@cy		cdef _Anchor anchor

		indexData = [ '\t<ul>\n' ]
		indent = 0

		def createIndent(indentCount):
			indexData.append('\t' * (indentCount + 1))

		def incIndent(count):
			curIndent = indent
			while count:
				curIndent += 1
				createIndent(curIndent)
				indexData.append('<ul>\n')
				count -= 1
			return curIndent

		def decIndent(count):
			curIndent = indent
			while count:
				createIndent(curIndent)
				indexData.append('</ul>\n')
				curIndent -= 1
				count -= 1
			return curIndent

		for anchor in anchors:
			if anchor.noIndex or not anchor.text:
				# No index item for this anchor
				continue

			if anchor.indent >= 0 and anchor.indent > indent:
				# Increase indent
				if anchor.indent > 1024:
					raise CMSException(500,
						"Anchor indent too big")
				indent = incIndent(anchor.indent - indent)
			elif anchor.indent >= 0 and anchor.indent < indent:
				# Decrease indent
				indent = decIndent(indent - anchor.indent)
			# Append the actual anchor data
			createIndent(indent)
			indexData.append('<li>')
			indexData.append('<a href="%s">%s</a>' %\
				(anchor.makeUrl(self),
				 anchor.text))
			indexData.append('</li>\n')

		# Close all indents
		decIndent(indent + 1)

		return "".join(indexData)

	# Insert the referenced indices
	def __processIndices(self, data):
#@cy		cdef _IndexRef indexRef

		offset = 0
		for indexRef in self.indexRefs:
			indexData = self.__createIndex(self.anchors)
			curOffset = offset + indexRef.charOffset
			data = data[0 : curOffset] +\
			       indexData +\
			       data[curOffset :]
			offset += len(indexData)
		return data

	def resolve(self, data, variables={}, pageIdent=None):
#@cy		cdef _ResolverRet ret

		if not data:
			return data
		self.__reset(variables, pageIdent)
		# Expand recursive statements
		ret = self.__expandRecStmts(data, 0, "")
		# Insert the indices
		data = self.__processIndices(ret.data)
		# Remove escapes
		data = self.unescape(data)
		return data

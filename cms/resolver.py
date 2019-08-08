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

def stackElem(name, lineno): #@nocy
#cdef _StackElem stackElem(str name, int64_t lineno): #@cy
#@cy	cdef _StackElem se
#@cy	cdef int64_t nlen
#@cy	cdef int64_t i

	se = _StackElem() #@nocy
	str2carray(se.name, name, MACRO_STACK_NAME_SIZE)
	se.lineno = lineno
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
#cdef _ResolverRet resolverRet(int64_t cons, str data): #@cy
#@cy	cdef _ResolverRet r
	r = _ResolverRet()
	r.cons = cons
	r.data = data
	return r

class CMSStatementResolver(object): #+cdef

	# Macro argument expansion: $1, $2, $3...
	macro_arg_re = re.compile(r'\$(\d+)', re.DOTALL)

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
		self.__macro_arg_re = self.macro_arg_re
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
		self.__callStack[0] = stackElem("content.html", 1)
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

	# Statement:  $(if CONDITION, THEN, ELSE)
	# Statement:  $(if CONDITION, THEN)
	# Returns THEN if CONDITION is nonempty after stripping whitespace.
	# Returns ELSE otherwise.
	def __stmt_if(self, d, dOffs):
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
		result = True
		firstArg = args[0]
		for arg in args[1:]:
			result = result and arg == firstArg
		if invert:
			result = not result
		return resolverRet(cons, (args[-1] if result else ""))

	# Statement:  $(eq A, B, ...)
	# Returns the last argument, if all stripped arguments are equal.
	# Returns an empty string otherwise.
	def __stmt_eq(self, d, dOffs):
		return self.__do_compare(d, dOffs, False)

	# Statement:  $(ne A, B, ...)
	# Returns the last argument, if not all stripped arguments are equal.
	# Returns an empty string otherwise.
	def __stmt_ne(self, d, dOffs):
		return self.__do_compare(d, dOffs, True)

	# Statement:  $(and A, B, ...)
	# Returns A, if all stripped arguments are non-empty strings.
	# Returns an empty string otherwise.
	def __stmt_and(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		return resolverRet(cons, (args[0] if all(args) else ""))

	# Statement:  $(or A, B, ...)
	# Returns the first stripped non-empty argument.
	# Returns an empty string, if there is no non-empty argument.
	def __stmt_or(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		nonempty = [ a for a in args if a ]
		return resolverRet(cons, (nonempty[0] if nonempty else ""))

	# Statement:  $(not A)
	# Returns 1, if A is an empty string after stripping.
	# Returns an empty string, if A is a non-empty stripped string.
	def __stmt_not(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if len(args) != 1:
			self.__stmtError("NOT: invalid args")
		return resolverRet(cons, ("" if args[0] else "1"))

	# Statement:  $(assert A, ...)
	# Raises a 500-assertion-failed exception, if any argument
	# is empty after stripping.
	# Returns an empty string, otherwise.
	def __stmt_assert(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if not all(args):
			self.__stmtError("ASSERT: failed")
		return resolverRet(cons, "")

	# Statement:  $(strip STRING)
	# Strip whitespace at the start and at the end of the string.
	def __stmt_strip(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		return resolverRet(cons, "".join(args))

	# Statement:  $(item STRING, N)
	# Statement:  $(item STRING, N, SEPARATOR)
	# Split a string into tokens and return the N'th token.
	# SEPARATOR defaults to whitespace.
	def __stmt_item(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in {2, 3}:
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

	# Statement:  $(substr STRING, START)
	# Statement:  $(substr STRING, START, END)
	# Returns a sub-string of STRING.
	def __stmt_substr(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in {2, 3}:
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

	# Statement:  $(sanitize STRING)
	# Sanitize a string.
	# Replaces all non-alphanumeric characters by an underscore. Forces lower-case.
	def __stmt_sanitize(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		string = "_".join(args)
		validChars = LOWERCASE + NUMBERS
		string = string.lower()
		string = "".join( c if c in validChars else '_' for c in string )
		string = re.sub(r'_+', '_', string).strip('_')
		return resolverRet(cons, string)

	# Statement:  $(file_exists RELATIVE_PATH)
	# Statement:  $(file_exists RELATIVE_PATH, DOES_NOT_EXIST)
	# Checks if a file exists relative to the wwwPath base.
	# Returns the path, if the file exists or an empty string if it doesn't.
	# If DOES_NOT_EXIST is specified, it returns this if the file doesn't exist.
	def __stmt_fileExists(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 1 and len(args) != 2:
			self.__stmtError("FILE_EXISTS: invalid args")
		relpath, enoent = args[0], args[1] if len(args) == 2 else ""
		try:
			exists = fs.exists(self.cms.wwwPath,
					   CMSPageIdent.validateSafePath(relpath))
		except (CMSException) as e:
			exists = False
		return resolverRet(cons, (relpath if exists else enoent))

	# Statement:  $(file_mdatet RELATIVE_PATH)
	# Statement:  $(file_mdatet RELATIVE_PATH, DOES_NOT_EXIST, FORMAT_STRING)
	# Returns the file modification time.
	# If the file does not exist, it returns DOES_NOT_EXIST or an empty string.
	# RELATIVE_PATH is relative to wwwPath.
	# FORMAT_STRING is an optional strftime format string.
	def __stmt_fileModDateTime(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in {1, 2, 3}:
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

	# Statement: $(index)
	# Returns the site index.
	def __stmt_index(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 1 or args[0]:
			self.__stmtError("INDEX: invalid args")
		self.indexRefs.append(_IndexRef(self.charCount))
		return resolverRet(cons, "")

	# Statement: $(anchor NAME, TEXT)
	# Statement: $(anchor NAME, TEXT, INDENT_LEVEL)
	# Statement: $(anchor NAME, TEXT, INDENT_LEVEL, NO_INDEX)
	# Sets an index-anchor
	def __stmt_anchor(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args
#@cy		cdef _Anchor anchor

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) < 2 or len(args) > 4:
			self.__stmtError("ANCHOR: invalid args")
		name, text = args[0:2]
		indent, noIndex = -1, False
		if len(args) >= 3:
			indent = args[2].strip()
			try:
				indent = int(indent) if indent else -1
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

	# Statement: $(pagelist BASEPAGE, ...)
	# Returns an <ul>-list of all sub-page names in the page.
	def __stmt_pagelist(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		try:
			basePageIdent = CMSPageIdent(args)
			subPages = self.cms.db.getSubPages(basePageIdent)
		except CMSException as e:
			self.__stmtError("PAGELIST: invalid base page name")
		html = [ '<ul>\n' ]
		for pagename, navlabel, prio in subPages:
			pageIdent = CMSPageIdent(basePageIdent + [pagename])
			pagetitle = self.cms.db.getPageTitle(pageIdent)
			html.append('\t<li><a href="%s">%s</a></li>\n' %\
				    (pageIdent.getUrl(urlBase = self.cms.urlBase),
				     pagetitle))
		html.append('</ul>')
		return resolverRet(cons, ''.join(html))

	# Statement: $(random)
	# Statement: $(random BEGIN)
	# Statement: $(random BEGIN, END)
	# Returns a random integer in the range from BEGIN to END
	# (including both end points)
	# BEGIN defaults to 0. END defaults to 65535.
	def __stmt_random(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, True)
		cons, args = a.cons, a.arguments
		if len(args) not in {0, 1, 2}:
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

	# Statement: $(randitem ITEM0, ITEM1, ...)
	# Returns one random item of its arguments.
	def __stmt_randitem(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) < 1:
			self.__stmtError("RANDITEM: too few args")
		return resolverRet(cons, random.choice(args))

	__validDomainChars = LOWERCASE + UPPERCASE + NUMBERS + "."

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

	# Statement: $(add A, B)
	# Returns A + B
	def __stmt_add(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("ADD: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a + b, args))

	# Statement: $(sub A, B)
	# Returns A - B
	def __stmt_sub(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("SUB: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a - b, args))

	# Statement: $(mul A, B)
	# Returns A * B
	def __stmt_mul(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("MUL: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a * b, args))

	# Statement: $(div A, B)
	# Returns A / B
	def __stmt_div(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("DIV: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a / b, args))

	# Statement: $(mod A, B)
	# Returns A % B
	def __stmt_mod(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("MOD: invalid args")
		return resolverRet(cons, self.__do_arith(lambda a, b: a % b, args))

	# Statement: $(round A)
	# Statement: $(round A, NDIGITS)
	# Returns A rounded
	def __stmt_round(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) not in {1, 2}:
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
				res = ("%." + str(n) + "f") % int(round(a, n))
		except (ValueError, TypeError) as e:
			self.__stmtError("ROUND: invalid value")
		return resolverRet(cons, res)

	# Statement: $(whois DOMAIN)
	# Executes whois and returns the text.
	def __stmt_whois(self, d, dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, dOffs, False)
		cons, args = a.cons, a.arguments
		if len(args) != 1:
			self.__stmtError("WHOIS: invalid args")
		domain = args[0]
		if [ c for c in domain if c not in self.__validDomainChars ]:
			self.__stmtError("WHOIS: invalid domain")
		try:
			import subprocess
			whois = subprocess.Popen([ "whois", domain ],
						 shell = False,
						 stdout = subprocess.PIPE)
			out, err = whois.communicate()
			out = out.decode("UTF-8", "strict")
		except UnicodeError as e:
			self.__stmtError("WHOIS: unicode error")
		except (OSError, ValueError) as e:
			self.__stmtError("WHOIS: execution error")
		return resolverRet(cons, out)

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

		# external programs
		"$(whois"	: __stmt_whois,
	}

	def __doMacro(self, macroname, d, dOffs): #@nocy
#@cy	cdef _ResolverRet __doMacro(self, str macroname, str d, int64_t dOffs):
#@cy		cdef _ArgParserRet a
#@cy		cdef _ResolverRet mRet
#@cy		cdef str macrodata
#@cy		cdef int64_t nrArguments

		if self.__callStackLen >= MACRO_STACK_SIZE:
			raise CMSException(500, "Exceed macro call stack depth")
		a = self.__parseArguments(d, dOffs, True)
		nrArguments = len(a.arguments)
		# Fetch the macro data from the database
		macrodata = None
		try:
			macrodata = self.cms.db.getMacro(macroname[1:],
							 self.pageIdent)
		except (CMSException) as e:
			if e.httpStatusCode == 404:
				raise CMSException(500,
					"Macro name '%s' contains "
					"invalid characters" % macroname)
		if not macrodata:
			return a.cons, ""  # Macro does not exist.
		# Expand the macro arguments ($1, $2, $3, ...)
		def expandArg(match):
#@cy			cdef int64_t nr
			nr = int(match.group(1))
			if nr >= 0:
				if nr >= 1 and nr <= nrArguments:
					return a.arguments[nr - 1]
				if nr == 0:
					return macroname
			return ""
		macrodata = self.__macro_arg_re.sub(expandArg, macrodata)

		# Resolve statements and recursive macro calls
		self.__callStack[self.__callStackLen] = stackElem(macroname, 1)
		self.__callStackLen += 1
		mRet = self.__expandRecStmts(macrodata, 0, "")
		macrodata = mRet.data
		self.__callStackLen -= 1

		return resolverRet(a.cons, macrodata)

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

		# Recursively expand statements and macro calls
		ret = []
		dEnd = len(d)
		i = dOffs
		while i < dEnd:
			c = d[i]
			res = None
			cons = 1
			if c == '\\': # Escaped characters
				# Keep escapes. They are removed later.
				if i + 1 < dEnd and\
				   d[i + 1] in self.__escapedChars:
					res = d[i:i+2]
					i += 1
			elif c == '\n':
				self.__callStack[self.__callStackLen - 1].lineno += 1
			elif c == '<' and d.startswith('<!---', i): # Comment
				end = d.find('--->', i)
				if end > i:
					strip_nl = 0
					# If comment is on a line of its own,
					# remove the line.
					if (i == 0 or d[i - 1] == '\n') and\
					   (end + 4 < dEnd and d[end + 4] == '\n'):
						strip_nl = 1
					cons, res = end - i + 4 + strip_nl, ""
			elif c in stopchars: # Stop character
				i += 1
				break
			elif c == '@': # Macro call
				end = d.find('(', i)
				if end > i:
					macroRet = self.__doMacro(d[i:end], d, end+1)
					cons, res = macroRet.cons, macroRet.data
					i = end + 1
			elif c == '$' and i + 1 < dEnd and d[i + 1] == '(': # Statement
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
			elif c == '$': # Variable
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

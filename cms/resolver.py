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
from functools import reduce
import random

__all__ = [
	"CMSStatementResolver",
]

class StackElem(object): # Call stack element
	def __init__(self, name):
		self.name = name
		self.lineno = 1

class IndexRef(object): # Index references
	def __init__(self, charOffset):
		self.charOffset = charOffset

class Anchor(object): # Anchor
	def __init__(self, name, text,
		     indent=-1, noIndex=False):
		self.name = name
		self.text = text
		self.indent = indent
		self.noIndex = noIndex

	def makeUrl(self, resolver):
		#TODO this does not work for sub pages
		return "%s#%s" % (
			CMSPageIdent((
				resolver.expandVariable("GROUP"),
				resolver.expandVariable("PAGE"))).getUrl(
				urlBase = resolver.cms.urlBase),
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

class CMSStatementResolver(object): #+cdef

	# Macro argument expansion: $1, $2, $3...
	macro_arg_re = re.compile(r'\$(\d+)', re.DOTALL)

	# Valid characters for variable names (without the leading $)
	VARNAME_CHARS = UPPERCASE + '_'

	__genericVars = {
		"DOMAIN"	: lambda self, n: self.cms.domain,
		"CMS_BASE"	: lambda self, n: self.cms.urlBase,
		"IMAGES_DIR"	: lambda self, n: self.cms.imagesDir,
		"THUMBS_DIR"	: lambda self, n: self.cms.urlBase + "/__thumbs",
		"DEBUG"		: lambda self, n: "1" if self.cms.debug else "",
		"__DUMPVARS__"	: lambda self, n: self.__dumpVars(),
	}

	def __init__(self, cms):
		self.cms = cms
		self.__reset()

	def __reset(self, variables = {}, pageIdent = None):
		self.variables = variables.copy()
		self.variables.update(self.__genericVars)
		self.pageIdent = pageIdent
		self.callStack = [ StackElem("content.html") ]
		self.charCount = 0
		self.indexRefs = []
		self.anchors = []

	def __stmtError(self, msg):
		pfx = ""
		if self.cms.debug:
			pfx = "%s:%d: " %\
				(self.callStack[-1].name,
				 self.callStack[-1].lineno)
		raise CMSException(500, pfx + msg)

	def expandVariable(self, name):
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

	__escapedChars = '\\,@$()'

	@classmethod
	def escape(cls, data):
		for c in cls.__escapedChars:
			data = data.replace(c, '\\' + c)
		return data

	@classmethod
	def unescape(cls, data):
		for c in cls.__escapedChars:
			data = data.replace('\\' + c, c)
		return data

	# Parse statement arguments.
	def __parseArguments(self, d, strip): #@nocy
#@cy	cdef _ArgParserRet __parseArguments(self, str d, _Bool strip):
#@cy		cdef _ResolverRet r
#@cy		cdef _ArgParserRet ret
#@cy		cdef list arguments
#@cy		cdef int64_t cons

		ret = _ArgParserRet()
		ret.cons = 0
		ret.arguments = []
		while ret.cons < len(d):
			r = self.__expandRecStmts(d[ret.cons:], ',)')
			ret.cons += r.cons
			ret.arguments.append(r.data.strip() if strip else r.data)
			if ret.cons <= 0 or d[ret.cons - 1] == ')':
				break
		return ret

	# Statement:  $(if CONDITION, THEN, ELSE)
	# Statement:  $(if CONDITION, THEN)
	# Returns THEN if CONDITION is nonempty after stripping whitespace.
	# Returns ELSE otherwise.
	def __stmt_if(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2 and len(args) != 3:
			self.__stmtError("IF: invalid number of arguments (%d)" %\
					 len(args))
		condition, b_then = args[0], args[1]
		b_else = args[2] if len(args) == 3 else ""
		result = b_then if condition.strip() else b_else
		return cons, result

	def __do_compare(self, d, invert):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, True)
		cons, args = a.cons, a.arguments
		result = reduce(lambda a, b: a and b == args[0],
				args[1:], True)
		result = not result if invert else result
		return cons, (args[-1] if result else "")

	# Statement:  $(eq A, B, ...)
	# Returns the last argument, if all stripped arguments are equal.
	# Returns an empty string otherwise.
	def __stmt_eq(self, d):
		return self.__do_compare(d, False)

	# Statement:  $(ne A, B, ...)
	# Returns the last argument, if not all stripped arguments are equal.
	# Returns an empty string otherwise.
	def __stmt_ne(self, d):
		return self.__do_compare(d, True)

	# Statement:  $(and A, B, ...)
	# Returns A, if all stripped arguments are non-empty strings.
	# Returns an empty string otherwise.
	def __stmt_and(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, True)
		cons, args = a.cons, a.arguments
		return cons, (args[0] if all(args) else "")

	# Statement:  $(or A, B, ...)
	# Returns the first stripped non-empty argument.
	# Returns an empty string, if there is no non-empty argument.
	def __stmt_or(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, True)
		cons, args = a.cons, a.arguments
		nonempty = [ a for a in args if a ]
		return cons, (nonempty[0] if nonempty else "")

	# Statement:  $(not A)
	# Returns 1, if A is an empty string after stripping.
	# Returns an empty string, if A is a non-empty stripped string.
	def __stmt_not(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, True)
		cons, args = a.cons, a.arguments
		if len(args) != 1:
			self.__stmtError("NOT: invalid args")
		return cons, ("" if args[0] else "1")

	# Statement:  $(assert A, ...)
	# Raises a 500-assertion-failed exception, if any argument
	# is empty after stripping.
	# Returns an empty string, otherwise.
	def __stmt_assert(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, True)
		cons, args = a.cons, a.arguments
		if not all(args):
			self.__stmtError("ASSERT: failed")
		return cons, ""

	# Statement:  $(strip STRING)
	# Strip whitespace at the start and at the end of the string.
	def __stmt_strip(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, True)
		cons, args = a.cons, a.arguments
		return cons, "".join(args)

	# Statement:  $(item STRING, N)
	# Statement:  $(item STRING, N, SEPARATOR)
	# Split a string into tokens and return the N'th token.
	# SEPARATOR defaults to whitespace.
	def __stmt_item(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
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
		return cons, token

	# Statement:  $(substr STRING, START)
	# Statement:  $(substr STRING, START, END)
	# Returns a sub-string of STRING.
	def __stmt_substr(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
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
		return cons, substr

	# Statement:  $(sanitize STRING)
	# Sanitize a string.
	# Replaces all non-alphanumeric characters by an underscore. Forces lower-case.
	def __stmt_sanitize(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		string = "_".join(args)
		validChars = LOWERCASE + NUMBERS
		string = string.lower()
		string = "".join( c if c in validChars else '_' for c in string )
		string = re.sub(r'_+', '_', string).strip('_')
		return cons, string

	# Statement:  $(file_exists RELATIVE_PATH)
	# Statement:  $(file_exists RELATIVE_PATH, DOES_NOT_EXIST)
	# Checks if a file exists relative to the wwwPath base.
	# Returns the path, if the file exists or an empty string if it doesn't.
	# If DOES_NOT_EXIST is specified, it returns this if the file doesn't exist.
	def __stmt_fileExists(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) != 1 and len(args) != 2:
			self.__stmtError("FILE_EXISTS: invalid args")
		relpath, enoent = args[0], args[1] if len(args) == 2 else ""
		try:
			exists = f_exists(self.cms.wwwPath,
					  CMSPageIdent.validateSafePath(relpath))
		except (CMSException) as e:
			exists = False
		return cons, (relpath if exists else enoent)

	# Statement:  $(file_mdatet RELATIVE_PATH)
	# Statement:  $(file_mdatet RELATIVE_PATH, DOES_NOT_EXIST, FORMAT_STRING)
	# Returns the file modification time.
	# If the file does not exist, it returns DOES_NOT_EXIST or and empty string.
	# RELATIVE_PATH is relative to wwwPath.
	# FORMAT_STRING is an optional strftime format string.
	def __stmt_fileModDateTime(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) not in {1, 2, 3}:
			self.__stmtError("FILE_MDATET: invalid args")
		relpath, enoent, fmtstr =\
			args[0],\
			args[1] if len(args) >= 2 else "",\
			args[2] if len(args) >= 3 else "%d %B %Y %H:%M (UTC)"
		try:
			stamp = f_mtime(self.cms.wwwPath,
					CMSPageIdent.validateSafePath(relpath))
		except (CMSException) as e:
			return cons, enoent
		return cons, stamp.strftime(fmtstr.strip())

	# Statement: $(index)
	# Returns the site index.
	def __stmt_index(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) != 1 or args[0]:
			self.__stmtError("INDEX: invalid args")
		self.indexRefs.append(IndexRef(self.charCount))
		return cons, ""

	# Statement: $(anchor NAME, TEXT)
	# Statement: $(anchor NAME, TEXT, INDENT_LEVEL)
	# Statement: $(anchor NAME, TEXT, INDENT_LEVEL, NO_INDEX)
	# Sets an index-anchor
	def __stmt_anchor(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
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
		anchor = Anchor(name, text, indent, noIndex)
		# Cache anchor for index creation
		self.anchors.append(anchor)
		# Create the anchor HTML
		return cons, '<a id="%s" href="%s">%s</a>' %\
			     (name, anchor.makeUrl(self), text)

	# Statement: $(pagelist BASEPAGE, ...)
	# Returns an <ul>-list of all sub-page names in the page.
	def __stmt_pagelist(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
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
		return cons, ''.join(html)

	# Statement: $(random)
	# Statement: $(random BEGIN)
	# Statement: $(random BEGIN, END)
	# Returns a random integer in the range from BEGIN to END
	# (including both end points)
	# BEGIN defaults to 0. END defaults to 65535.
	def __stmt_random(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, True)
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
		return cons, '%d' % rnd

	# Statement: $(randitem ITEM0, ITEM1, ...)
	# Returns one random item of its arguments.
	def __stmt_randitem(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) < 1:
			self.__stmtError("RANDITEM: too few args")
		return cons, random.choice(args)

	__validDomainChars = LOWERCASE + UPPERCASE + NUMBERS + "."

	def __do_arith(self, oper, args):
		try:
			a = float(args[0])
		except ValueError as e:
			a = 0.0
		try:
			b = float(args[1])
		except ValueError as e:
			b = 0.0
		res = oper(a, b)
		rounded = int(round(res))
		return ("%f" % res) if (abs(res - rounded) >= 0.000001)\
			else str(rounded)

	# Statement: $(add A, B)
	# Returns A + B
	def __stmt_add(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("ADD: invalid args")
		return cons, self.__do_arith(lambda a, b: a + b, args)

	# Statement: $(sub A, B)
	# Returns A - B
	def __stmt_sub(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("SUB: invalid args")
		return cons, self.__do_arith(lambda a, b: a - b, args)

	# Statement: $(mul A, B)
	# Returns A * B
	def __stmt_mul(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("MUL: invalid args")
		return cons, self.__do_arith(lambda a, b: a * b, args)

	# Statement: $(div A, B)
	# Returns A / B
	def __stmt_div(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("DIV: invalid args")
		return cons, self.__do_arith(lambda a, b: a / b, args)

	# Statement: $(mod A, B)
	# Returns A % B
	def __stmt_mod(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) != 2:
			self.__stmtError("MOD: invalid args")
		return cons, self.__do_arith(lambda a, b: a % b, args)

	# Statement: $(round A)
	# Statement: $(round A, NDIGITS)
	# Returns A rounded
	def __stmt_round(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
		cons, args = a.cons, a.arguments
		if len(args) not in {1, 2}:
			self.__stmtError("ROUND: invalid args")
		try:
			a = float(args[0])
		except ValueError as e:
			a = 0.0
		try:
			if len(args) == 1:
				res = str(int(round(a)))
			else:
				try:
					n = int(args[1])
				except ValueError as e:
					n = 0
				res = ("%." + str(n) + "f") % int(round(a, n))
		except (ValueError, TypeError) as e:
			self.__stmtError("ROUND: invalid value")
		return cons, res

	# Statement: $(whois DOMAIN)
	# Executes whois and returns the text.
	def __stmt_whois(self, d):
#@cy		cdef _ArgParserRet a
#@cy		cdef int64_t cons
#@cy		cdef list args

		a = self.__parseArguments(d, False)
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
			out = out.decode("UTF-8")
		except UnicodeError as e:
			self.__stmtError("WHOIS: unicode error")
		except (OSError, ValueError) as e:
			self.__stmtError("WHOIS: execution error")
		return cons, out

	# statement handlers
	__handlers = {
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

	def __doMacro(self, macroname, d): #@nocy
#@cy	cdef _ResolverRet __doMacro(self, str macroname, str d):
#@cy		cdef _ResolverRet ret
#@cy		cdef _ArgParserRet a
#@cy		cdef str macrodata

		if len(self.callStack) > 16:
			raise CMSException(500, "Exceed macro call stack depth")
		a = self.__parseArguments(d, True)
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
			nr = int(match.group(1), 10)
			if nr >= 1 and nr <= len(a.arguments):
				return a.arguments[nr - 1]
			return macroname if nr == 0 else ""
		macrodata = self.macro_arg_re.sub(expandArg, macrodata)
		# Resolve statements and recursive macro calls
		self.callStack.append(StackElem(macroname))
		macrodata = self.__resolve(macrodata)
		self.callStack.pop()

		ret = _ResolverRet()
		ret.cons = a.cons
		ret.data = macrodata
		return ret

	def __expandRecStmts(self, d, stopchars): #@nocy
#@cy	cdef _ResolverRet __expandRecStmts(self, str d, str stopchars):
#@cy		cdef int64_t i
#@cy		cdef int64_t end
#@cy		cdef int64_t cons
#@cy		cdef str stmtName
#@cy		cdef str escapedChars
#@cy		cdef dict handlers
#@cy		cdef list ret
#@cy		cdef _ResolverRet macroRet
#@cy		cdef _ResolverRet retObj

		# Recursively expand statements and macro calls
		escapedChars = self.__escapedChars
		handlers = self.__handlers
		retObj = _ResolverRet()
		ret, i = [], 0
		while i < len(d):
			cons, res = 1, d[i]
			if d[i] == '\\': # Escaped characters
				# Keep escapes. They are removed later.
				if i + 1 < len(d) and\
				   d[i + 1] in escapedChars:
					res = d[i:i+2]
					i += 1
			elif d[i] == '\n':
				self.callStack[-1].lineno += 1
			elif d.startswith('<!---', i): # Comment
				end = d.find('--->', i)
				if end > i:
					strip_nl = 0
					# If comment is on a line of its own,
					# remove the line.
					if (i == 0 or d[i - 1] == '\n') and\
					   (end + 4 < len(d) and d[end + 4] == '\n'):
						strip_nl = 1
					cons, res = end - i + 4 + strip_nl, ""
			elif d[i] in stopchars: # Stop character
				i += 1
				break
			elif d[i] == '@': # Macro call
				end = d.find('(', i)
				if end > i:
					macroRet = self.__doMacro(
						d[i:end],
						d[end+1:])
					cons, res = macroRet.cons, macroRet.data
					i = end + 1
			elif d.startswith('$(', i): # Statement
				end = findAny(d, ' )', i)
				if end > i:
					stmtName = d[i:end]
					if stmtName in handlers:
						h = handlers[stmtName]
						i = end + 1 if d[end] == ' ' else end
					else:
						h = lambda _self, x: (cons, res) # nop
				cons, res = h(self, d[i:])
			elif d[i] == '$': # Variable
				end = findNot(d, self.VARNAME_CHARS, i + 1)
				if end > i + 1:
					res = self.expandVariable(d[i+1:end])
					cons = end - i
			ret.append(res)
			i += cons
			self.charCount += len(res)
		if stopchars and i >= len(d) and d[-1] not in stopchars:
			self.__stmtError("Unterminated statement")

		retObj.data = "".join(ret)
		self.charCount -= len(retObj.data)
		retObj.cons = i
		return retObj

	# Create an index
	def __createIndex(self, anchors):
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
		offset = 0
		for indexRef in self.indexRefs:
			indexData = self.__createIndex(self.anchors)
			curOffset = offset + indexRef.charOffset
			data = data[0 : curOffset] +\
			       indexData +\
			       data[curOffset :]
			offset += len(indexData)
		return data

	def __resolve(self, data): #@nocy
#@cy	cdef str __resolve(self, str data):
#@cy		cdef _ResolverRet ret

		# Expand recursive statements
		ret = self.__expandRecStmts(data, "")
		return ret.data

	def resolve(self, data, variables = {}, pageIdent = None):
		if not data:
			return data
		self.__reset(variables, pageIdent)
		data = self.__resolve(data)
		# Insert the indices
		data = self.__processIndices(data)
		# Remove escapes
		data = self.unescape(data)
		return data

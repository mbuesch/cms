# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2011-2016 Michael Buesch <m@bues.ch>
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

import sys
if sys.version_info[0] < 3 or sys.version_info[1] < 3:
	raise Exception("Need Python 3.3 or later")

import os
from stat import S_ISDIR
from datetime import datetime
import re
import PIL.Image as Image
from io import BytesIO
import urllib.request, urllib.parse, urllib.error
import cgi
from functools import reduce
import random
import importlib.machinery


UPPERCASE = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
LOWERCASE = 'abcdefghijklmnopqrstuvwxyz'
NUMBERS   = '0123456789'

# Find the index in 'string' that is _not_ in 'template'.
# Start search at 'idx'.
# Returns -1 on failure to find.
def findNot(string, template, idx=0):
	while idx < len(string):
		if string[idx] not in template:
			return idx
		idx += 1
	return -1

# Find the index in 'string' that matches _any_ character in 'template'.
# Start search at 'idx'.
# Returns -1 on failure to find.
def findAny(string, template, idx=0):
	while idx < len(string):
		if string[idx] in template:
			return idx
		idx += 1
	return -1

def htmlEscape(string):
	return cgi.escape(string, True)

def stringBool(string, default=False):
	s = string.lower()
	if s in ("true", "yes", "on"):
		return True
	if s in ("false", "no", "off"):
		return False
	try:
		return bool(int(s, 10))
	except ValueError:
		return default

# Create a path string from path element strings.
def mkpath(*path_elements):
	# Do not use os.path.join, because it discards elements, if
	# one element begins with a separator (= is absolute).
	return os.path.sep.join(path_elements)

def f_exists(*path_elements):
	try:
		os.stat(mkpath(*path_elements))
	except OSError:
		return False
	return True

def f_exists_nonempty(*path_elements):
	if f_exists(*path_elements):
		return bool(f_read(*path_elements).strip())
	return False

def f_read(*path_elements):
	try:
		with open(mkpath(*path_elements), "rb") as fd:
			return fd.read().decode("UTF-8")
	except IOError:
		return ""
	except UnicodeError:
		raise CMSException(500, "Unicode decode error")

def f_read_int(*path_elements):
	data = f_read(*path_elements)
	try:
		return int(data.strip(), 10)
	except ValueError:
		return None

def f_mtime(*path_elements):
	try:
		return datetime.utcfromtimestamp(os.stat(mkpath(*path_elements)).st_mtime)
	except OSError:
		raise CMSException(404)

def f_mtime_nofail(*path_elements):
	try:
		return f_mtime(*path_elements)
	except CMSException:
		return datetime.utcnow()

def f_subdirList(*path_elements):
	def dirfilter(dentry):
		if dentry.startswith("."):
			return False # Omit ".", ".." and hidden entries
		if dentry.startswith("__"):
			return False # Omit system folders/files.
		try:
			if not S_ISDIR(os.stat(mkpath(path, dentry)).st_mode):
				return False
		except OSError:
			return False
		return True
	path = mkpath(*path_elements)
	try:
		return [ dentry for dentry in os.listdir(path) \
			 if dirfilter(dentry) ]
	except OSError:
		return []

class CMSPageIdent(list):
	# Page identifier.

	__pageFileName_re	= re.compile(
		r'^(.*)((?:\.html?)|(?:\.py)|(?:\.php))$', re.DOTALL)
	__indexPages		= {"", "index"}

	# Parse a page identifier from a string.
	@classmethod
	def parse(cls, path, maxPathLen = 512, maxIdentDepth = 32):
		if len(path) > maxPathLen:
			raise CMSException(400, "Invalid URL")

		pageIdent = cls()

		# Strip whitespace and slashes
		path = path.strip(' \t/')

		# Remove page file extensions like .html and such.
		m = cls.__pageFileName_re.match(path)
		if m:
			path = m.group(1)

		# Use the ident elements, if this is not the root page.
		if path not in cls.__indexPages:
			pageIdent.extend(path.split("/"))

		if len(pageIdent) > maxIdentDepth:
			raise CMSException(400, "Invalid URL")

		return pageIdent

	__pathSep = os.path.sep
	__validPathChars = LOWERCASE + UPPERCASE + NUMBERS + "-_."

	# Validate a path component. Avoid any directory change.
	# Raises CMSException on failure.
	@classmethod
	def validateSafePathComponent(cls, pcomp):
		if pcomp.startswith('.'):
			# No ".", ".." and hidden files.
			raise CMSException(404, "Invalid page path")
		if [ c for c in pcomp if c not in cls.__validPathChars ]:
			raise CMSException(404, "Invalid page path")
		return pcomp

	# Validate a path. Avoid going back in the hierarchy (. and ..)
	# Raises CMSException on failure.
	@classmethod
	def validateSafePath(cls, path):
		for pcomp in path.split(cls.__pathSep):
			cls.validateSafePathComponent(pcomp)
		return path

	# Validate a page name.
	# Raises CMSException on failure.
	# If allowSysNames is True, system names starting with "__" are allowed.
	@classmethod
	def validateName(cls, name, allowSysNames = False):
		if name.startswith("__") and not allowSysNames:
			# Page names with __ are system folders.
			raise CMSException(404, "Invalid page name")
		return cls.validateSafePathComponent(name)

	def __init__(self, *args):
		list.__init__(self, *args)
		self.__allValidated = False

	# Validate all page identifier name components.
	# (Do not allow system name components)
	def __validateAll(self):
		if not self.__allValidated:
			for pcomp in self:
				self.validateName(pcomp)
			# Remember that we validated.
			# Note that this assumes no components are added later!
			self.__allValidated = True

	# Get one page identifier component by index.
	def get(self, index, default = None, allowSysNames = False):
		try:
			return self.validateName(self[index],
						 allowSysNames)
		except IndexError:
			return default

	# Get the page identifier as URL.
	def getUrl(self, protocol = None, domain = None,
		   urlBase = None, pageSuffix = ".html"):
		self.__validateAll()
		url = []
		if protocol:
			url.append(protocol + ":/")
		if domain:
			url.append(domain)
		if urlBase:
			url.append(urlBase.strip("/"))
		url.extend(self)
		if not protocol and not domain:
			url.insert(0, "")
		url = "/".join(url)
		if self and pageSuffix:
			url += pageSuffix
		return url

	# Get the page identifier as filesystem path.
	def getFilesystemPath(self, rstrip = 0):
		self.__validateAll()
		if self:
			if rstrip:
				pcomps = self[ : 0 - rstrip]
				if pcomps:
					return mkpath(*pcomps)
				return ""
			return mkpath(*self)
		return ""

	# Test if this identifier starts with the same elements
	# as another one.
	def startswith(self, other):
		return other is not None and\
		       len(self) >= len(other) and\
		       self[ : len(other)] == other

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
		return name if default is None else default

	def getPostHandler(self, pageIdent):
		path = mkpath(self.pageBase, pageIdent.getFilesystemPath())
		handlerModFile = mkpath(path, "post.py")
		if not f_exists(handlerModFile):
			return None
		try:
			loader = importlib.machinery.SourceFileLoader(
				re.sub(r"[^A-Za-z]", "_", handlerModFile),
				handlerModFile)
			mod = loader.load_module()
		except OSError:
			return None
		if not hasattr(mod, "post"):
			return None
		return mod

class CMSStatementResolver(object):
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
			return "%s#%s" % (
				CMSPageIdent((
					resolver.expandVariable("GROUP"),
					resolver.expandVariable("PAGE"))).getUrl(
					urlBase = resolver.cms.urlBase),
				self.name)

	def __init__(self, cms):
		self.cms = cms
		self.__reset()

	def __reset(self, variables = {}, pageIdent = None):
		self.variables = variables.copy()
		self.variables.update(self.__genericVars)
		self.pageIdent = pageIdent
		self.callStack = [ self.StackElem("content.html") ]
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
			try:
				value = value(self, name)
			except (TypeError) as e:
				pass
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

	__escapedChars = ('\\', ',', '@', '$', '(', ')')

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
	# Returns (consumed-characters-count, arguments) tuple.
	def __parseArguments(self, d, strip=False):
		arguments, cons = [], 0
		while cons < len(d):
			c, arg = self.__expandRecStmts(d[cons:], ',)')
			cons += c
			arguments.append(arg.strip() if strip else arg)
			if cons <= 0 or d[cons - 1] == ')':
				break
		return cons, arguments

	# Statement:  $(if CONDITION, THEN, ELSE)
	# Statement:  $(if CONDITION, THEN)
	# Returns THEN if CONDITION is nonempty after stripping whitespace.
	# Returns ELSE otherwise.
	def __stmt_if(self, d):
		cons, args = self.__parseArguments(d)
		if len(args) != 2 and len(args) != 3:
			self.__stmtError("IF: invalid number of arguments (%d)" %\
					 len(args))
		condition, b_then = args[0], args[1]
		b_else = args[2] if len(args) == 3 else ""
		result = b_then if condition.strip() else b_else
		return cons, result

	def __do_compare(self, d, invert):
		cons, args = self.__parseArguments(d, strip=True)
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
		cons, args = self.__parseArguments(d, strip=True)
		return cons, (args[0] if all(args) else "")

	# Statement:  $(or A, B, ...)
	# Returns the first stripped non-empty argument.
	# Returns an empty string, if there is no non-empty argument.
	def __stmt_or(self, d):
		cons, args = self.__parseArguments(d, strip=True)
		nonempty = [ a for a in args if a ]
		return cons, (nonempty[0] if nonempty else "")

	# Statement:  $(not A)
	# Returns 1, if A is an empty string after stripping.
	# Returns an empty string, if A is a non-empty stripped string.
	def __stmt_not(self, d):
		cons, args = self.__parseArguments(d, strip=True)
		if len(args) != 1:
			self.__stmtError("NOT: invalid args")
		return cons, ("" if args[0] else "1")

	# Statement:  $(assert A, ...)
	# Raises a 500-assertion-failed exception, if any argument
	# is empty after stripping.
	# Returns an empty string, otherwise.
	def __stmt_assert(self, d):
		cons, args = self.__parseArguments(d, strip=True)
		if not all(args):
			self.__stmtError("ASSERT: failed")
		return cons, ""

	# Statement:  $(strip STRING)
	# Strip whitespace at the start and at the end of the string.
	def __stmt_strip(self, d):
		cons, args = self.__parseArguments(d, strip=True)
		return cons, "".join(args)

	# Statement:  $(item STRING, N)
	# Statement:  $(item STRING, N, SEPARATOR)
	# Split a string into tokens and return the N'th token.
	# SEPARATOR defaults to whitespace.
	def __stmt_item(self, d):
		cons, args = self.__parseArguments(d)
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
		cons, args = self.__parseArguments(d)
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
		cons, args = self.__parseArguments(d)
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
		cons, args = self.__parseArguments(d)
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
		cons, args = self.__parseArguments(d)
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
		cons, args = self.__parseArguments(d)
		if len(args) != 1 or args[0]:
			self.__stmtError("INDEX: invalid args")
		self.indexRefs.append(self.IndexRef(self.charCount))
		return cons, ""

	# Statement: $(anchor NAME, TEXT)
	# Statement: $(anchor NAME, TEXT, INDENT_LEVEL)
	# Statement: $(anchor NAME, TEXT, INDENT_LEVEL, NO_INDEX)
	# Sets an index-anchor
	def __stmt_anchor(self, d):
		cons, args = self.__parseArguments(d)
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
		anchor = self.Anchor(name, text, indent, noIndex)
		# Cache anchor for index creation
		self.anchors.append(anchor)
		# Create the anchor HTML
		return cons, '<a name="%s" href="%s">%s</a>' %\
			     (name, anchor.makeUrl(self), text)

	# Statement: $(pagelist BASEPAGE, ...)
	# Returns an <ul>-list of all sub-page names in the page.
	def __stmt_pagelist(self, d):
		cons, args = self.__parseArguments(d)
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
		cons, args = self.__parseArguments(d, strip=True)
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
		cons, args = self.__parseArguments(d)
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
		cons, args = self.__parseArguments(d)
		if len(args) != 2:
			self.__stmtError("ADD: invalid args")
		return cons, self.__do_arith(lambda a, b: a + b, args)

	# Statement: $(sub A, B)
	# Returns A - B
	def __stmt_sub(self, d):
		cons, args = self.__parseArguments(d)
		if len(args) != 2:
			self.__stmtError("SUB: invalid args")
		return cons, self.__do_arith(lambda a, b: a - b, args)

	# Statement: $(mul A, B)
	# Returns A * B
	def __stmt_mul(self, d):
		cons, args = self.__parseArguments(d)
		if len(args) != 2:
			self.__stmtError("MUL: invalid args")
		return cons, self.__do_arith(lambda a, b: a * b, args)

	# Statement: $(div A, B)
	# Returns A / B
	def __stmt_div(self, d):
		cons, args = self.__parseArguments(d)
		if len(args) != 2:
			self.__stmtError("DIV: invalid args")
		return cons, self.__do_arith(lambda a, b: a / b, args)

	# Statement: $(mod A, B)
	# Returns A % B
	def __stmt_mod(self, d):
		cons, args = self.__parseArguments(d)
		if len(args) != 2:
			self.__stmtError("MOD: invalid args")
		return cons, self.__do_arith(lambda a, b: a % b, args)

	# Statement: $(round A)
	# Statement: $(round A, NDIGITS)
	# Returns A rounded
	def __stmt_round(self, d):
		cons, args = self.__parseArguments(d)
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
		cons, args = self.__parseArguments(d)
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

	def __doMacro(self, macroname, d):
		if len(self.callStack) > 16:
			raise CMSException(500, "Exceed macro call stack depth")
		cons, arguments = self.__parseArguments(d, strip=True)
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
			return cons, ""  # Macro does not exist.
		# Expand the macro arguments ($1, $2, $3, ...)
		def expandArg(match):
			nr = int(match.group(1), 10)
			if nr >= 1 and nr <= len(arguments):
				return arguments[nr - 1]
			return macroname if nr == 0 else ""
		macrodata = self.macro_arg_re.sub(expandArg, macrodata)
		# Resolve statements and recursive macro calls
		self.callStack.append(self.StackElem(macroname))
		macrodata = self.__resolve(macrodata)
		self.callStack.pop()
		return cons, macrodata

	def __expandRecStmts(self, d, stopchars=""):
		# Recursively expand statements and macro calls
		ret, i = [], 0
		while i < len(d):
			cons, res = 1, d[i]
			if d[i] == '\\': # Escaped characters
				# Keep escapes. They are removed later.
				if i + 1 < len(d) and\
				   d[i + 1] in self.__escapedChars:
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
					cons, res = self.__doMacro(
						d[i:end],
						d[end+1:])
					i = end + 1
			elif d.startswith('$(', i): # Statement
				h = lambda _self, x: (cons, res) # nop
				end = findAny(d, ' )', i)
				if end > i:
					try:
						h = self.__handlers[d[i:end]]
						i = end + 1 if d[end] == ' ' else end
					except KeyError: pass
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
		retData = "".join(ret)
		self.charCount -= len(retData)
		return i, retData

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

	def __resolve(self, data):
		# Expand recursive statements
		unused, data = self.__expandRecStmts(data)
		return data

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

class CMSQuery(object):
	def __init__(self, queryDict):
		self.queryDict = queryDict

	def get(self, name, default=""):
		try:
			return self.queryDict[name][-1]
		except (KeyError, IndexError) as e:
			return default

	def getInt(self, name, default=0):
		try:
			return int(self.get(name, str(int(default))), 10)
		except (ValueError) as e:
			return default

	def getBool(self, name, default=False):
		string = self.get(name, str(bool(default)))
		return stringBool(string, default)

class CMS(object):
	# Main CMS entry point.

	__rootPageIdent = CMSPageIdent()

	def __init__(self,
		     dbPath,
		     wwwPath,
		     imagesDir="/images",
		     domain="example.com",
		     urlBase="/cms",
		     cssUrlPath="/cms.css",
		     debug=False):
		# dbPath => Unix path to the database directory.
		# wwwPath => Unix path to the static www data.
		# imagesDir => Subdirectory path, based on wwwPath, to
		#	the images directory.
		# domain => The site domain name.
		# urlBase => URL base component to the HTTP server CMS mapping.
		# cssUrlBase => URL subpath to the CSS.
		# debug => Enable/disable debugging
		self.wwwPath = wwwPath
		self.imagesDir = imagesDir
		self.domain = domain
		self.urlBase = urlBase
		self.cssUrlPath = cssUrlPath
		self.debug = debug

		self.db = CMSDatabase(dbPath)
		self.resolver = CMSStatementResolver(self)

	def shutdown(self):
		pass

	def __genHtmlHeader(self, title, additional = ""):
		header = """<?xml version="1.0" encoding="UTF-8" ?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">
<html xmlns="http://www.w3.org/1999/xhtml" lang="en" xml:lang="en">
<head>
	<meta http-equiv="content-type" content="text/html; charset=utf-8" />
	<meta name="robots" content="all" />
	<meta name="date" content="%s" />
	<meta name="generator" content="WSGI/Python CMS" />
	<!--
		Generated by "cms.py - simple WSGI/Python based CMS script"
		https://bues.ch/cgit/cms.git
	-->
	<title>%s</title>
	<link rel="stylesheet" href="%s" type="text/css" />
	%s
</head>
<body>
"""		% (
			datetime.now().isoformat(),
			title,
			self.cssUrlPath,
			additional or ""
		)
		return header

	def __genHtmlFooter(self):
		footer = """
</body>
</html>
"""
		return footer

	def __genNavElem(self, body, basePageIdent, activePageIdent, indent = 0):
		if self.db.getNavStop(basePageIdent):
			return
		subPages = self.db.getSubPages(basePageIdent)
		if not subPages:
			return
		tabs = '\t' + '\t' * indent
		if indent > 0:
			body.append('%s<div class="navelems">' % tabs)
		for pageElement in subPages:
			pagename, pagelabel, pageprio = pageElement
			if pagelabel:
				body.append('%s\t<div class="%s"> '
					    '<!-- %d -->' % (
					    tabs,
					    "navelem" if indent > 0 else "navgroup",
					    pageprio))
				if indent <= 0:
					body.append('%s\t\t<div class="navhead">' %\
						    tabs)
				subPageIdent = CMSPageIdent(basePageIdent + [pagename])
				isActive = activePageIdent.startswith(subPageIdent)
				if isActive:
					body.append('%s\t\t<div class="navactive">' %\
						    tabs)
				body.append('%s\t\t<a href="%s">%s</a>' %\
					    (tabs,
					     subPageIdent.getUrl(urlBase = self.urlBase),
					     pagelabel))
				if isActive:
					body.append('%s\t\t</div> '
						    '<!-- class="navactive" -->' %\
						    tabs)
				if indent <= 0:
					body.append('%s\t\t</div>' % tabs)

				self.__genNavElem(body, subPageIdent,
						  activePageIdent, indent + 2)

				body.append('%s\t</div>' % tabs)
		if indent > 0:
			body.append('%s</div>' % tabs)

	def __genHtmlBody(self, pageIdent, pageTitle, pageData,
			  protocol,
			  stamp=None, genCheckerLinks=True):
		body = []

		# Generate logo / title bar
		body.append('<div class="titlebar">')
		body.append('\t<div class="logo">')
		body.append('\t\t<a href="%s">' % self.urlBase)
		body.append('\t\t\t<img alt="logo" src="/logo.png" />')
		body.append('\t\t</a>')
		body.append('\t</div>')
		body.append('\t<div class="title">%s</div>' % pageTitle)
		body.append('</div>\n')

		# Generate navigation bar
		body.append('<div class="navbar">')
		body.append('\t<div class="navgroups">')
		body.append('\t\t<div class="navhome">')
		rootActive = not pageIdent
		if rootActive:
			body.append('\t\t<div class="navactive">')
		body.append('\t\t\t<a href="%s">%s</a>' %\
			    (self.__rootPageIdent.getUrl(urlBase = self.urlBase),
			     self.db.getString("home")))
		if rootActive:
			body.append('\t\t</div> <!-- class="navactive" -->')
		body.append('\t\t</div>')
		self.__genNavElem(body, self.__rootPageIdent, pageIdent)
		body.append('\t</div>')
		body.append('</div>\n')

		body.append('<div class="main">\n') # Main body start

		# Page content
		body.append('<!-- BEGIN: page content -->')
		body.append(pageData)
		body.append('<!-- END: page content -->\n')

		if stamp:
			# Last-modified date
			body.append('\t<div class="modifystamp">')
			body.append(stamp.strftime('\t\tUpdated: %A %d %B %Y %H:%M (UTC)'))
			body.append('\t</div>')

		if protocol != "https":
			# SSL
			body.append('\t<div class="ssl">')
			body.append('\t\t<a href="%s">%s</a>' % (
				    pageIdent.getUrl("https", self.domain,
						     self.urlBase),
				    self.db.getString("ssl-encrypted")))
			body.append('\t</div>')

		if genCheckerLinks:
			# Checker links
			pageUrlQuoted = urllib.parse.quote_plus(
				pageIdent.getUrl("http", self.domain,
						 self.urlBase))
			body.append('\t<div class="checker">')
			checkerUrl = "http://validator.w3.org/check?"\
				     "uri=" + pageUrlQuoted + "&amp;"\
				     "charset=%28detect+automatically%29&amp;"\
				     "doctype=Inline&amp;group=0&amp;"\
				     "user-agent=W3C_Validator%2F1.2"
			body.append('\t\t<a href="%s">%s</a> /' %\
				    (checkerUrl, self.db.getString("checker-xhtml")))
			checkerUrl = "http://jigsaw.w3.org/css-validator/validator?"\
				     "uri=" + pageUrlQuoted + "&amp;profile=css3&amp;"\
				     "usermedium=all&amp;warning=1&amp;"\
				     "vextwarning=true&amp;lang=en"
			body.append('\t\t<a href="%s">%s</a>' %\
				    (checkerUrl, self.db.getString("checker-css")))
			body.append('\t</div>\n')

		body.append('</div>\n') # Main body end

		return "\n".join(body)

	def __getImageThumbnail(self, imagename, query, protocol):
		if not imagename:
			raise CMSException(404)
		width = query.getInt("w", 300)
		height = query.getInt("h", 300)
		qual = query.getInt("q", 1)
		qualities = {
			0 : Image.NEAREST,
			1 : Image.BILINEAR,
			2 : Image.BICUBIC,
			3 : Image.ANTIALIAS,
		}
		try:
			qual = qualities[qual]
		except (KeyError) as e:
			qual = qualities[1]
		try:
			img = Image.open(mkpath(self.wwwPath, self.imagesDir,
				CMSPageIdent.validateSafePathComponent(imagename)))
			img.thumbnail((width, height), qual)
			output = BytesIO()
			img.save(output, "JPEG")
			data = output.getvalue()
		except (IOError) as e:
			raise CMSException(404)
		return data, "image/jpeg"

	def __getHtmlPage(self, pageIdent, query, protocol):
		pageTitle, pageData, stamp = self.db.getPage(pageIdent)
		if not pageData:
			raise CMSException(404)

		resolverVariables = {
			"PROTOCOL"	: lambda r, n: protocol,
			"GROUP"		: lambda r, n: pageIdent.get(0),
			"PAGE"		: lambda r, n: pageIdent.get(1),
		}
		resolve = self.resolver.resolve
		for k, v in query.queryDict.items():
			k, v = k.upper(), v[-1]
			resolverVariables["Q_" + k] = CMSStatementResolver.escape(htmlEscape(v))
			resolverVariables["QRAW_" + k] = CMSStatementResolver.escape(v)
		pageTitle = resolve(pageTitle, resolverVariables, pageIdent)
		resolverVariables["TITLE"] = lambda r, n: pageTitle
		pageData = resolve(pageData, resolverVariables, pageIdent)
		extraHeader = resolve(self.db.getHeader(pageIdent),
				      resolverVariables, pageIdent)

		data = [self.__genHtmlHeader(pageTitle, extraHeader)]
		data.append(self.__genHtmlBody(pageIdent,
					       pageTitle, pageData,
					       protocol, stamp))
		data.append(self.__genHtmlFooter())
		try:
			return "".join(data).encode("UTF-8"), \
			       "text/html; charset=UTF-8"
		except UnicodeError as e:
			raise CMSException(500, "Unicode encode error")

	def __generate(self, path, query, protocol):
		pageIdent = CMSPageIdent.parse(path)
		if pageIdent.get(0, allowSysNames = True) == "__thumbs":
			return self.__getImageThumbnail(pageIdent.get(1), query, protocol)
		return self.__getHtmlPage(pageIdent, query, protocol)

	def get(self, path, query={}, protocol="http"):
		query = CMSQuery(query)
		return self.__generate(path, query, protocol)

	def __post(self, path, query, body, bodyType, protocol):
		pageIdent = CMSPageIdent.parse(path)
		postHandler = self.db.getPostHandler(pageIdent)
		if not postHandler:
			raise CMSException(405)
		try:
			ret = postHandler.post(query, body, bodyType, protocol)
		except Exception as e:
			msg = ""
			if self.debug:
				msg = " " + str(e)
			msg = msg.encode("UTF-8", "ignore")
			return (b"Failed to run POST handler." + msg,
				"text/plain")
		if ret is None:
			return self.__generate(path, query, protocol)
		assert(isinstance(ret, tuple) and len(ret) == 2)
		assert((isinstance(ret[0], bytes) or isinstance(ret[0], bytearray)) and\
		       isinstance(ret[1], str))
		return ret

	def post(self, path, query={},
		 body=b"", bodyType="text/plain",
		 protocol="http"):
		raise CMSException(405) #TODO disabled

		query = CMSQuery(query)
		return self.__post(path, query, body, bodyType, protocol)

	def __doGetErrorPage(self, cmsExcept, protocol):
		resolverVariables = {
			"PROTOCOL"		: lambda r, n: protocol,
			"GROUP"			: lambda r, n: "_nogroup_",
			"PAGE"			: lambda r, n: "_nopage_",
			"HTTP_STATUS"		: lambda r, n: cmsExcept.httpStatus,
			"HTTP_STATUS_CODE"	: lambda r, n: str(cmsExcept.httpStatusCode),
			"ERROR_MESSAGE"		: lambda r, n: CMSStatementResolver.escape(htmlEscape(cmsExcept.message)),
		}
		pageHeader = cmsExcept.getHtmlHeader(self.db)
		pageHeader = self.resolver.resolve(pageHeader, resolverVariables)
		pageData = cmsExcept.getHtmlBody(self.db)
		pageData = self.resolver.resolve(pageData, resolverVariables)
		httpHeaders = cmsExcept.getHttpHeaders(
			lambda s: self.resolver.resolve(s, resolverVariables))
		data = [self.__genHtmlHeader(cmsExcept.httpStatus,
					     additional=pageHeader)]
		data.append(self.__genHtmlBody(CMSPageIdent(("_nogroup_", "_nopage_")),
					       cmsExcept.httpStatus,
					       pageData,
					       protocol,
					       genCheckerLinks=False))
		data.append(self.__genHtmlFooter())
		return "".join(data), "text/html; charset=UTF-8", httpHeaders

	def getErrorPage(self, cmsExcept, protocol="http"):
		try:
			data, mime, headers = self.__doGetErrorPage(cmsExcept, protocol)
		except (CMSException) as e:
			data = "Error in exception handler: %s %s" % \
				(e.httpStatus, e.message)
			mime, headers = "text/plain; charset=UTF-8", ()
		try:
			return data.encode("UTF-8"), mime, headers
		except UnicodeError as e:
			# Whoops. All is lost.
			raise CMSException(500, "Unicode encode error")
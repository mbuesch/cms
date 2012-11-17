#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2011-2012 Michael Buesch <m@bues.ch>
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
import os
from stat import S_ISDIR
from datetime import datetime
import re
import Image
from StringIO import StringIO
import urllib
import cgi


UPPERCASE = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
LOWERCASE = 'abcdefghijklmnopqrstuvwxyz'
NUMBERS   = '0123456789'

def findNot(string, template, idx=0):
	while idx < len(string):
		if string[idx] not in template:
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

PATHSEP = "/"

def mkpath(*path_elements):
	return PATHSEP.join(path_elements)

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
		fd = open(mkpath(*path_elements), "rb")
		data = fd.read()
		fd.close()
		return data
	except IOError:
		return ""

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
		return datetime.utcnow()

def f_subdirList(*path_elements):
	def dirfilter(dentry):
		if dentry.startswith("."):
			return False # Omit ".", ".." and hidden entries
		try:
			if not S_ISDIR(os.stat(mkpath(path, dentry)).st_mode):
				return False
		except OSError:
			return False
		return True
	path = mkpath(*path_elements)
	try:
		return filter(dirfilter, os.listdir(path))
	except OSError:
		return []

def validateSafePathComponent(pcomp):
	# Validate a path component. Avoid any directory change.
	# Raises CMSException on failure.
	if pcomp.startswith('.'):
		# No ".", ".." and hidden files.
		raise CMSException(404)
	validChars = LOWERCASE + UPPERCASE + NUMBERS + "-_."
	if [ c for c in pcomp if c not in validChars ]:
		raise CMSException(404)
	return pcomp

def validateSafePath(path):
	# Validate a path. Avoid going back in the hierarchy (. and ..)
	# Raises CMSException on failure.
	for pcomp in path.split(PATHSEP):
		validateSafePathComponent(pcomp)
	return path

validateName = validateSafePathComponent

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
	def __init__(self, basePath):
		self.pageBase = mkpath(basePath, "pages")
		self.macroBase = mkpath(basePath, "macros")
		self.stringBase = mkpath(basePath, "strings")

	def getPage(self, groupname, pagename):
		path = mkpath(self.pageBase,
			      validateName(groupname),
			      validateName(pagename))
		redirect = f_read(path, "redirect").strip()
		if redirect:
			raise CMSException301(redirect)
		title = f_read(path, "title").strip()
		if not title:
			title = f_read(path, "nav_label").strip()
		data = f_read(path, "content.html")
		stamp = f_mtime(path, "content.html")
		return (title, data, stamp)

	def getGroupNames(self):
		# Returns list of (groupname, navlabel, prio)
		res = []
		for groupname in f_subdirList(self.pageBase):
			path = mkpath(self.pageBase, groupname)
			if f_exists(path, "hidden"):
				continue
			navlabel = f_read(path, "nav_label").strip()
			prio = f_read_int(path, "priority")
			res.append( (groupname, navlabel, prio) )
		return res

	def getPageNames(self, groupname):
		# Returns list of (pagename, navlabel, prio)
		res = []
		gpath = mkpath(self.pageBase, validateName(groupname))
		for pagename in f_subdirList(gpath):
			path = mkpath(gpath, pagename)
			if f_exists(path, "hidden") or \
			   f_exists_nonempty(path, "redirect"):
				continue
			navlabel = f_read(path, "nav_label").strip()
			prio = f_read_int(path, "priority")
			res.append( (pagename, navlabel, prio) )
		return res

	def getMacro(self, name):
		data = f_read(self.macroBase, validateName(name))
		return '\n'.join( l for l in data.splitlines() if l )

	def getString(self, name, default=None):
		name = validateName(name)
		string = f_read(self.stringBase, name).strip()
		if string:
			return string
		return name if default is None else default

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

	def __init__(self, cms):
		self.cms = cms
		self.__reset()

	def __reset(self, variables={}):
		self.variables = variables.copy()
		self.variables.update(self.__genericVars)
		self.callStack = [ self.StackElem("content.html") ]

	def __stmtError(self, msg):
		pfx = ""
		if self.cms.debug:
			pfx = "%s:%d: " %\
				(self.callStack[-1].name,
				 self.callStack[-1].lineno)
		raise CMSException(500, pfx + msg)

	def __expandVariable(self, name):
		try:
			value = self.variables[name]
			try:
				value = value(self, name)
			except (TypeError), e:
				pass
			return str(value)
		except (KeyError, TypeError), e:
			return ""

	def __dumpVars(self, force=False):
		if not force and not self.cms.debug:
			return ""
		ret, names = [], self.variables.keys()
		names.sort()
		for name in names:
			if name == "__DUMPVARS__":
				value = "-- variable dump --"
			else:
				value = self.__expandVariable(name)
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
			self.__stmtError("IF: invalid args")
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
					  validateSafePath(relpath))
		except (CMSException), e:
			exists = False
		return cons, (relpath if exists else enoent)

	__handlers = {
		"$(if"		: __stmt_if,
		"$(eq"		: __stmt_eq,
		"$(ne"		: __stmt_ne,
		"$(and"		: __stmt_and,
		"$(or"		: __stmt_or,
		"$(not"		: __stmt_not,
		"$(assert"	: __stmt_assert,
		"$(strip"	: __stmt_strip,
		"$(sanitize"	: __stmt_sanitize,
		"$(file_exists"	: __stmt_fileExists,
	}

	def __doMacro(self, macroname, d):
		if len(self.callStack) > 16:
			raise CMSException(500, "Exceed macro call stack depth")
		cons, arguments = self.__parseArguments(d, strip=True)
		# Fetch the macro data from the database
		macrodata = None
		try:
			macrodata = self.cms.db.getMacro(macroname[1:])
		except (CMSException), e:
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
				end = d.find(' ', i)
				if end > i:
					try:
						h = self.__handlers[d[i:end]]
						i = end + 1
					except KeyError: pass
				cons, res = h(self, d[i:])
			elif d[i] == '$': # Variable
				end = findNot(d, self.VARNAME_CHARS, i + 1)
				if end > i + 1:
					res = self.__expandVariable(d[i+1:end])
					cons = end - i
			ret.append(res)
			i += cons
		if stopchars and i >= len(d) and d[-1] not in stopchars:
			self.__stmtError("Unterminated statement")
		return i, "".join(ret)

	def __resolve(self, data):
		# Expand recursive statements
		unused, data = self.__expandRecStmts(data)
		# Remove escapes
		data = self.unescape(data)
		return data

	def resolve(self, data, variables={}):
		self.__reset(variables)
		return self.__resolve(data)

class CMSQuery(object):
	def __init__(self, queryDict):
		self.queryDict = queryDict

	def get(self, name, default=""):
		try:
			return self.queryDict[name][-1]
		except (KeyError, IndexError), e:
			return default

	def getInt(self, name, default=0):
		try:
			return int(self.get(name, str(int(default))), 10)
		except (ValueError), e:
			return default

	def getBool(self, name, default=False):
		string = self.get(name, str(bool(default)))
		return stringBool(string, default)

class CMS(object):
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

	def __genHtmlHeader(self, title, additional=""):
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
		http://bues.ch/gitweb?p=cms.git;a=summary
	-->
	<title>%s</title>
	<link rel="stylesheet" href="%s" type="text/css" />
	%s
</head>
<body>
"""		%\
		(datetime.now().isoformat(), title, self.cssUrlPath, additional)
		return header

	def __genHtmlFooter(self):
		footer = """
</body>
</html>
"""
		return footer

	def __makePageUrl(self, groupname, pagename):
		if groupname:
			return "/".join( (self.urlBase, groupname, pagename + ".html") )
		return self.urlBase

	def __makeFullPageUrl(self, groupname, pagename, protocol="http"):
		return "%s://%s%s" % (protocol, self.domain,
				      self.__makePageUrl(groupname, pagename))

	def __genHtmlBody(self, groupname, pagename, pageTitle, pageData,
			  stamp=None,
			  genSslLinks=True, genCheckerLinks=True):
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
		if not groupname:
			body.append('\t\t<div class="navactive">')
		body.append('\t\t\t<a href="%s">%s</a>' %\
			    (self.__makePageUrl(None, None),
			     self.db.getString("home")))
		if not groupname:
			body.append('\t\t</div> <!-- class="navactive" -->')
		body.append('\t\t</div>')
		navGroups = self.db.getGroupNames()
		def getNavPrio(element):
			name, label, prio = element
			if prio is None:
				prio = 500
			return "%03d_%s" % (prio, label)
		navGroups.sort(key=getNavPrio)
		for navGroupElement in navGroups:
			navgroupname, navgrouplabel, navgroupprio = navGroupElement
			body.append('\t\t<div class="navgroup"> '
				    '<!-- %s -->' % getNavPrio(navGroupElement))
			if navgrouplabel:
				body.append('\t\t\t<div class="navhead">%s</div>' % navgrouplabel)
			body.append('\t\t\t<div class="navelems">')
			navPages = self.db.getPageNames(navgroupname)
			navPages.sort(key=getNavPrio)
			for navPageElement in navPages:
				(navpagename, navpagelabel, navpageprio) = navPageElement
				body.append('\t\t\t\t<div class="navelem"> '
					    '<!-- %s -->' %\
					    getNavPrio(navPageElement))
				if navgroupname == groupname and\
				   navpagename == pagename:
					body.append('\t\t\t\t<div class="navactive">')
				url = self.__makePageUrl(navgroupname, navpagename)
				body.append('\t\t\t\t\t<a href="%s">%s</a>' %\
					    (url, navpagelabel))
				if navgroupname == groupname and\
				   navpagename == pagename:
					body.append('\t\t\t\t</div> <!-- class="navactive" -->')
				body.append('\t\t\t\t</div>')
			body.append('\t\t\t</div>')
			body.append('\t\t</div>')
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

		if genSslLinks:
			# SSL
			body.append('\t<div class="ssl">')
			body.append('\t\t<a href="%s">%s</a>' %\
				    (self.__makeFullPageUrl(groupname, pagename,
							    protocol="https"),
				     self.db.getString("ssl-encrypted")))
			body.append('\t</div>')

		if genCheckerLinks:
			# Checker links
			pageUrlQuoted = urllib.quote_plus(
				self.__makeFullPageUrl(groupname, pagename))
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

	def __parsePagePath(self, path):
		path = path.strip().lstrip('/')
		for suffix in ('.html', '.htm', '.php'):
			if path.endswith(suffix):
				path = path[:-len(suffix)]
				break
		groupname, pagename = '', ''
		if path not in ('', 'index'):
			path = path.split('/')
			if len(path) == 2:
				groupname, pagename = path[0], path[1]
			if not groupname or not pagename:
				raise CMSException(404)
		return groupname, pagename

	def __getImageThumbnail(self, imagename, query):
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
		except (KeyError), e:
			qual = qualities[1]
		try:
			img = Image.open(mkpath(self.wwwPath, self.imagesDir,
					validateSafePathComponent(imagename)))
			img.thumbnail((width, height), qual)
			output = StringIO()
			img.save(output, "JPEG")
			data = output.getvalue()
		except (IOError), e:
			raise CMSException(404)
		return data, "image/jpeg"

	def __getHtmlPage(self, groupname, pagename, query):
		pageTitle, pageData, stamp = self.db.getPage(groupname, pagename)
		if not pageData:
			raise CMSException(404)
		resolverVariables = {
			"GROUP"	: lambda r, n: groupname,
			"PAGE"	: lambda r, n: pagename,
		}
		for k, v in query.queryDict.iteritems():
			k, v = k.upper(), v[-1]
			resolverVariables["Q_" + k] = CMSStatementResolver.escape(htmlEscape(v))
			resolverVariables["QRAW_" + k] = CMSStatementResolver.escape(v)
		pageTitle = self.resolver.resolve(pageTitle, resolverVariables)
		pageData = self.resolver.resolve(pageData, resolverVariables)
		data = [self.__genHtmlHeader(pageTitle)]
		data.append(self.__genHtmlBody(groupname, pagename,
					       pageTitle, pageData, stamp))
		data.append(self.__genHtmlFooter())
		return "".join(data), "text/html"

	def __generate(self, path, query):
		groupname, pagename = self.__parsePagePath(path)
		if groupname == "__thumbs":
			return self.__getImageThumbnail(pagename, query)
		return self.__getHtmlPage(groupname, pagename, query)

	def get(self, path, query={}):
		query = CMSQuery(query)
		return self.__generate(path, query)

	def post(self, path, query={}):
		raise CMSException(405)

	def __doGetErrorPage(self, cmsExcept):
		resolverVariables = {
			"GROUP"			: lambda r, n: "__nogroup",
			"PAGE"			: lambda r, n: "__nopage",
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
		data.append(self.__genHtmlBody('__nogroup', '__nopage',
					       cmsExcept.httpStatus,
					       pageData,
					       genSslLinks=False,
					       genCheckerLinks=False))
		data.append(self.__genHtmlFooter())
		return "".join(data), "text/html", httpHeaders

	def getErrorPage(self, cmsExcept):
		try:
			return self.__doGetErrorPage(cmsExcept)
		except (CMSException), e:
			data = "Error in exception handler: %s %s" % \
				(e.httpStatus, e.message)
			return data, "text/plain", ()

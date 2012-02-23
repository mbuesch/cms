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
from beaker.cache import cache_region, cache_regions
import urllib

CACHE_BASEDIR = "/tmp/www-cache/"

cache_regions.update(
	{
#		"html" : {
#			"expire"	: 3600,
#			"type"		: "memory",
#		},
		"image" : {
			"expire"	: 2592000, # 30 days
			"type"		: "file",
			"data_dir"	: CACHE_BASEDIR + "/image/data",
			"lock_dir"	: CACHE_BASEDIR + "/image/lock",
		},
	},
)


def stringBool(string):
	s = string.lower()
	if s in ("true", "yes", "on"):
		return True
	if s in ("false", "no", "off"):
		return False
	try:
		return bool(int(s, 10))
	except ValueError:
		return False

PATHSEP = "/"

def mkpath(*path_elements):
	return PATHSEP.join(path_elements)

def f_exists(*path_elements):
	try:
		os.stat(mkpath(*path_elements))
	except OSError:
		return False
	return True

def f_read(*path_elements):
	try:
		return file(mkpath(*path_elements), "rb").read()
	except IOError:
		return ""

def f_read_int(*path_elements):
	data = f_read(*path_elements)
	try:
		return int(data.strip(), 10)
	except ValueError:
		return None

def f_check_disablefile(*path_elements):
	if not f_exists(*path_elements):
		return False
	data = f_read(*path_elements)
	if not data:
		return True # Empty file
	return stringBool(data)

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
	validChars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-_."
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
	statusTab = {
		400: "400 Bad Request",
		404: "404 Not Found",
		405: "405 Method Not Allowed",
		409: "409 Conflict",
		500: "500 Internal Server Error",
	}

	def __init__(self, httpStatus=500, message=""):
		self.httpStatusNumber = httpStatus
		try:
			self.httpStatus = self.statusTab[httpStatus]
		except (KeyError), e:
			self.httpStatus = self.statusTab[500]
		self.message = message

class CMSDatabase:
	def __init__(self, basePath):
		self.pageBase = mkpath(basePath, "pages")
		self.macroBase = mkpath(basePath, "macros")
		self.stringBase = mkpath(basePath, "strings")

	def getPage(self, groupname, pagename):
		path = mkpath(self.pageBase,
			      validateName(groupname),
			      validateName(pagename))
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
			if f_check_disablefile(path, "disabled"):
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
			if f_check_disablefile(path, "disabled"):
				continue
			navlabel = f_read(path, "nav_label").strip()
			prio = f_read_int(path, "priority")
			res.append( (pagename, navlabel, prio) )
		return res

	def getMacro(self, name):
		def macroLineFilter(line):
			line = line.strip()
			return line and not line.startswith("#")
		lines = f_read(self.macroBase, validateName(name)).splitlines()
		return "\n".join(filter(macroLineFilter, lines))

	def getString(self, name, default=None):
		name = validateName(name)
		string = f_read(self.stringBase, name).strip()
		if string:
			return string
		return name if default is None else default

class CMSStatementResolver(object):
	# Macro call: @name(param1, param2, ...)
	macro_re = re.compile(r'@(\w+)\(([^\)]*)\)', re.DOTALL)
	# Macro parameter expansion: $1, $2, $3...
	macro_param_re = re.compile(r'\$(\d+)', re.DOTALL)
	# Content comment <!--- comment --->
	comment_re = re.compile(r'<!---(.*)--->', re.DOTALL)

	def __init__(self, cms):
		self.cms = cms

	def setNames(self, groupname, pagename):
		self.groupname, self.pagename = groupname, pagename

	# Statement:  $(if CONDITION, THEN, ELSE)
	# Statement:  $(if CONDITION, THEN)
	def __stmt_if(self, d):
		# CONDITION
		i, condition = self.__expandRecStmts(d, ',')
		# THEN branch
		cons, b_then = self.__expandRecStmts(d[i:], ',)')
		i += cons
		if i <= 0 or d[i - 1] == ')':  # No ELSE branch
			b_else = ""
		else:  # Have ELSE branch. Expand it.
			cons, b_else = self.__expandRecStmts(d[i:], ')')
			i += cons
		result = b_then if condition.strip() else b_else
		return i, result

	# Statement:  $(strip STRING)
	# Strip whitespace at the start and at the end of the string.
	def __stmt_strip(self, d):
		i, string = self.__expandRecStmts(d, ')')
		return i, string.strip()

	# Statement:  $(sanitize STRING)
	# Sanitize a string.
	# Replaces all non-alphanumeric characters by an underscore. Forces lower-case.
	def __stmt_sanitize(self, d):
		cons, string = self.__expandRecStmts(d, ')')
		validChars = "abcdefghijklmnopqrstuvwxyz1234567890"
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
		i, relpath = self.__expandRecStmts(d, ',)')
		if i <= 0 or d[i - 1] == ')':
			enoent = "" # Empty string for nonexisting case.
		else:  # Predefined string for nonexisting case.
			cons, enoent = self.__expandRecStmts(d[i:], ')')
			i += cons
		try:
			exists = f_exists(self.cms.wwwPath,
					  validateSafePath(relpath))
		except (CMSException), e:
			exists = False
		return i, (relpath if exists else enoent)

	__stmtHandlers = (
		("$(if ",		__stmt_if),
		("$(strip ",		__stmt_strip),
		("$(sanitize ",		__stmt_sanitize),
		("$(file_exists ",	__stmt_fileExists),
	)

	def __expandRecStmts(self, d, stopchars=""):
		# Recursively expand statements
		ret, i = [], 0
		while i < len(d):
			di = d[i]
			cons, res = 1, di
			if di in stopchars:
				i += cons
				break
			try:
				if di == '$':
					handler = lambda x: (cons, res) # nop
					for (stmt, h) in self.__stmtHandlers:
						if d[i:i+len(stmt)] == stmt:
							handler, i = h, i + len(stmt)
							break
					cons, res = handler(self, d[i:])
			except IndexError: pass
			ret.append(res)
			i += cons
		return i, "".join(ret)

	def __resolveStatements(self, data):
		# Remove comments
		data = self.comment_re.sub("", data)
		# Expand variables
		exvars = (
			("$GROUP"	, self.groupname),
			("$PAGE"	, self.pagename),
		)
		for var, value in exvars:
			data = data.replace(var, value)
		# Expand recursive statements
		unused, data = self.__expandRecStmts(data)
		return data

	def __resolveOneMacro(self, match, recurseLevel):
		if recurseLevel > 16:
			raise CMSException(500, "Exceed macro recurse depth")
		def expandParam(match):
			pnumber = int(match.group(1), 10)
			if pnumber >= 1 and pnumber <= len(parameters):
				return parameters[pnumber - 1]
			if pnumber == 0:
				return macroname
			return "" # Param not given or invalid
		macroname = match.group(1)
		parameters = [ p.strip() for p in match.group(2).split(",") ]
		# Get the raw macro value
		macrovalue = self.cms.db.getMacro(macroname)
		if not macrovalue:
			return "\n<!-- WARNING: INVALID MACRO USED: %s -->\n" %\
				match.group(0)
		# Expand the macro parameters
		macrovalue = self.macro_param_re.sub(expandParam, macrovalue)
		# Resolve statements and recursive macro calls
		return self.resolve(macrovalue, recurseLevel)

	def resolve(self, data, recurseLevel=0):
		data = self.__resolveStatements(data)
		return self.macro_re.sub(
			lambda m: self.__resolveOneMacro(m, recurseLevel + 1),
			data)

class CMS:
	def __init__(self,
		     dbPath,
		     wwwPath,
		     imagesDir="/images",
		     domain="example.com",
		     urlBase="/cms",
		     cssUrlPath="/cms.css",
		     cssPrintUrlPath="/cms-print.css"):
		# dbPath => Unix path to the database directory.
		# wwwPath => Unix path to the static www data.
		# imagesDir => Subdirectory path, based on wwwPath, to
		#	the images directory.
		# domain => The site domain name.
		# urlBase => URL base component to the HTTP server CMS mapping.
		# cssUrlBase => URL subpath to the main CSS.
		# cssPrintUrlBase => URL subpath to the "print" CSS.
		self.wwwPath = wwwPath
		self.imagesDir = imagesDir
		self.domain = domain
		self.urlBase = urlBase
		self.cssUrlPath = cssUrlPath
		self.cssPrintUrlPath = cssPrintUrlPath

		self.db = CMSDatabase(dbPath)
		self.resolver = CMSStatementResolver(self)

	def shutdown(self):
		pass

	def __genHtmlHeader(self, title, cssUrlPath):
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
</head>
<body>
"""		%\
		(datetime.now().isoformat(), title, cssUrlPath)
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

	def __genHtmlBody(self, groupname, pagename, pageTitle, pageData, stamp):
		body = []

		# Generate logo / title bar
		body.append('<a href="%s">' % self.urlBase)
		body.append('\t<img class="logo" alt="logo" src="/logo.png" />')
		body.append('</a>\n')
		body.append('<h1 class="titlebar">%s</h1>\n' % pageTitle)

		# Generate navigation bar
		body.append('<div class="navbar">\n')
		body.append('\t<div class="navgroup">')
		body.append('\t\t<div class="navhome">')
		if not groupname:
			body.append('\t\t<div class="navactive">')
		body.append('\t\t\t<a href="%s">%s</a>' %\
			    (self.__makePageUrl(None, None),
			     self.db.getString("home")))
		if not groupname:
			body.append('\t\t</div> <!-- class="navactive" -->')
		body.append('\t\t</div>')
		body.append('\t</div>\n')
		navGroups = self.db.getGroupNames()
		def getNavPrio(element):
			name, label, prio = element
			if prio is None:
				prio = 500
			return "%03d_%s" % (prio, label)
		navGroups.sort(key=getNavPrio)
		for navGroupElement in navGroups:
			navgroupname, navgrouplabel, navgroupprio = navGroupElement
			body.append('\t<div class="navgroup"> '
				    '<!-- %s -->' % getNavPrio(navGroupElement))
			if navgrouplabel:
				body.append('\t\t<div class="navhead">%s</div>' % navgrouplabel)
			navPages = self.db.getPageNames(navgroupname)
			navPages.sort(key=getNavPrio)
			for navPageElement in navPages:
				(navpagename, navpagelabel, navpageprio) = navPageElement
				body.append('\t\t<div class="navelem"> '
					    '<!-- %s -->' %\
					    getNavPrio(navPageElement))
				if navgroupname == groupname and\
				   navpagename == pagename:
					body.append('\t\t<div class="navactive">')
				url = self.__makePageUrl(navgroupname, navpagename)
				body.append('\t\t\t<a href="%s">%s</a>' %\
					    (url, navpagelabel))
				if navgroupname == groupname and\
				   navpagename == pagename:
					body.append('\t\t</div> <!-- class="navactive" -->')
				body.append('\t\t</div>')
			body.append('\t</div>\n')
		body.append('</div>\n')

		body.append('<div class="main">\n') # Main body start

		# Page content
		body.append('<!-- BEGIN: page content -->')
		body.append(pageData)
		body.append('<!-- END: page content -->\n')

		# Last-modified date
		body.append('\t<div class="modifystamp">')
		body.append(stamp.strftime('\t\tUpdated: %A %d %B %Y %H:%M (UTC)'))
		body.append('\t</div>')

		# Format links
		body.append('\t<div class="formatlinks">')
		url = self.__makePageUrl(groupname, pagename) + "?print=1"
		body.append('\t\t<a href="%s" target="_blank">%s</a>' %\
			    (url, self.db.getString("printer-layout")))
		body.append('\t</div>')

		# SSL
		body.append('\t<div class="ssl">')
		body.append('\t\t<a href="%s">%s</a>' %\
			    (self.__makeFullPageUrl(groupname, pagename,
						    protocol="https"),
			     self.db.getString("ssl-encrypted")))
		body.append('\t</div>')

		# Checker links
		pageUrlQuoted = urllib.quote_plus(self.__makeFullPageUrl(groupname, pagename))
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
		rootpages = (
			"/",
			"/index.htm",
			"/index.html",
			"/index.php",
		)
		path = path.strip()
		if not path.startswith("/"):
			path = "/" + path
		groupname = ""
		pagename = ""
		if path not in rootpages:
			path = path.split("/")
			if len(path) > 3:
				raise CMSException(404)
			try:
				groupname = path[1]
				pagename = path[2]
			except IndexError: pass
			for suffix in (".html", ".htm", ".php"):
				if pagename.endswith(suffix):
					pagename = pagename[:-len(suffix)]
			if not groupname or not pagename:
				raise CMSException(404)
		return (groupname, pagename)

	@cache_region("image", "thumbnail")
	def __getImageThumbnail(self, imagename, query):
		try:
			width = int(query["w"][0], 10)
			height = int(query["h"][0], 10)
		except (KeyError, IndexError, ValueError), e:
			raise CMSException(400)
		try:
			img = Image.open(mkpath(self.wwwPath, self.imagesDir,
					validateSafePathComponent(imagename)))
			img.thumbnail((width, height), Image.ANTIALIAS)
			output = StringIO()
			img.save(output, "JPEG")
			data = output.getvalue()
		except (IOError), e:
			raise CMSException(404)
		return (data, "image/jpeg")

#	@cache_region("html", "page")
	def __getHtmlPage(self, groupname, pagename, cssUrlPath):
		(pageTitle, pageData, stamp) = self.db.getPage(groupname, pagename)
		if not pageData:
			raise CMSException(404)
		self.resolver.setNames(groupname, pagename)
		pageData = self.resolver.resolve(pageData)
		data = [self.__genHtmlHeader(pageTitle, cssUrlPath)]
		data.append(self.__genHtmlBody(groupname, pagename,
					       pageTitle, pageData, stamp))
		data.append(self.__genHtmlFooter())
		return ("".join(data), "text/html")

	def __generate(self, path, cssUrlPath, query):
		(groupname, pagename) = self.__parsePagePath(path)
		if groupname == "__thumbs":
			return self.__getImageThumbnail(pagename, query)
		return self.__getHtmlPage(groupname, pagename, cssUrlPath)

	def get(self, path, query={}):
		cssUrlPath = self.cssUrlPath
		try:
			if stringBool(query["print"][0]):
				cssUrlPath = self.cssPrintUrlPath
		except (KeyError, IndexError), e: pass
		return self.__generate(path, cssUrlPath, query)

	def post(self, path, query={}):
		raise CMSException(405)

	def getErrorPage(self, cmsExcept):
		html = [self.__genHtmlHeader("Error - %s" % cmsExcept.httpStatus,
					     self.cssUrlPath)]
		html.append('<h1>An error occurred</h1>')
		html.append('<p>We\'re sorry for the inconvenience. ')
		html.append('The page could not be accessed, because:</p>')
		html.append('<p style="font-size: xx-large;">')
		html.append(cmsExcept.httpStatus)
		if cmsExcept.message:
			html.append('<br />')
			html.append(cmsExcept.message)
		html.append('</p>')
		html.append('<p>You may visit the <a href="%s" style="font-size: xx-large;">main page</a>' %\
			    self.__makePageUrl(None, None))
		html.append('and navigate manually to your desired target page.</p>')
		html.append(self.__genHtmlFooter())

		return ("\n".join(html), "text/html")

#!/usr/bin/env python
#
#   Copyright (C) 2011 Michael Buesch <m@bues.ch>
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

cache_regions.update(
	{
		"html" : {
			"expire"	: 3600,
			"type"		: "memory",
		},
		"image" : {
			"expire"	: 86400,
			"type"		: "file",
			"data_dir"	: "/tmp/www-cache/data",
			"lock_dir"	: "/tmp/www-cache/lock",
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
		return bool(int(s))
	except ValueError:
		return False

PATHSEP = "/"

def mkpath(*path_elements):
	return "/".join(path_elements)

def f_exists(*path_elements):
	try:
		os.stat(PATHSEP.join(path_elements))
	except OSError:
		return False
	return True

def f_read(*path_elements):
	try:
		return file(PATHSEP.join(path_elements), "rb").read()
	except IOError:
		return ""

def f_check_disablefile(*path_elements):
	path = PATHSEP.join(path_elements)
	if not f_exists(path):
		return False
	data = f_read(path)
	if not data:
		return True # Empty file
	return stringBool(data)

def f_mtime(*path_elements):
	try:
		return datetime.fromtimestamp(os.stat(PATHSEP.join(path_elements)).st_mtime)
	except OSError:
		return datetime.now()

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
	path = PATHSEP.join(path_elements)
	try:
		return filter(dirfilter, os.listdir(path))
	except OSError:
		return []

def validateName(name):
	# Validate a name string for use as safe path component
	# Raises CMSException on failure.
	if name.startswith('.'):
		# No ".", ".." and hidden files.
		raise CMSException(404)
	validChars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-_."
	for char in name:
		if char not in validChars:
			raise CMSException(404)
	return name

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
			self.httpStatus = str(httpStatus)
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
		# Returns list of (groupname, navlabel)
		res = []
		for groupname in f_subdirList(self.pageBase):
			path = mkpath(self.pageBase, groupname)
			if f_check_disablefile(path, "disabled"):
				continue
			navlabel = f_read(path, "nav_label").strip()
			res.append( (groupname, navlabel) )
		return res

	def getPageNames(self, groupname):
		# Returns list of (pagename, navlabel)
		res = []
		gpath = mkpath(self.pageBase, validateName(groupname))
		for pagename in f_subdirList(gpath):
			path = mkpath(gpath, pagename)
			if f_check_disablefile(path, "disabled"):
				continue
			navlabel = f_read(path, "nav_label").strip()
			res.append( (pagename, navlabel) )
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

class CMS:
	# Macro call: @name(param1, param2, ...)
	macro_re = re.compile(r'@(\w+)\(([^\)]*)\)', re.DOTALL)
	# Macro parameter expansion: $1, $2, $3...
	macro_param_re = re.compile(r'\$(\d+)', re.DOTALL)
	# Macro if-conditional: $(if COND,THEN,ELSE)
	macro_ifcond_re = re.compile(r'\$\(if\s+([^,\)]*),([^,\)]*)(?:,([^,\)]*))?\)', re.DOTALL)
	# Macro string sanitize: $(sanitize STRING)
	macro_strsan_re = re.compile(r'\$\(sanitize\s+([^,\)]*)\)', re.DOTALL)

	def __init__(self,
		     dbPath,
		     imagesPath,
		     domain,
		     basePath="/cms",
		     cssPath="/cms.css",
		     cssPrintPath="/cms-print.css"):
		self.domain = domain
		self.imagesPath = imagesPath
		self.basePath = basePath
		self.cssPath = cssPath
		self.cssPrintPath = cssPrintPath

		self.db = CMSDatabase(dbPath)

	def shutdown(self):
		pass

	def __genHtmlHeader(self, title, cssPath):
		header = """<?xml version="1.0" encoding="UTF-8" ?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">
<html xmlns="http://www.w3.org/1999/xhtml" lang="en" xml:lang="en">
<head>
	<meta http-equiv="content-type" content="text/html; charset=utf-8" />
	<meta name="robots" content="all" />
	<meta name="generator" content="Simple CMS script" />
	<meta name="date" content="%s" />
	<!-- Python powered  http://python.org/about/ -->
	<title>%s</title>
	<link rel="stylesheet" href="%s" type="text/css" />
</head>
<body>
"""		%\
		(datetime.now().isoformat(), title, cssPath)
		return header

	def __genHtmlFooter(self):
		footer = """
</body>
</html>
"""
		return footer

	def __makePageUrl(self, groupname, pagename):
		if groupname:
			return "/".join( (self.basePath, groupname, pagename + ".html") )
		return self.basePath

	def __makeFullPageUrl(self, groupname, pagename, protocol="http"):
		return "%s://%s%s" % (protocol, self.domain,
				      self.__makePageUrl(groupname, pagename))

	def __genHtmlBody(self, groupname, pagename, pageTitle, pageData, stamp):
		body = []

		# Generate logo / title bar
		body.append('<a href="%s">' % self.basePath)
		body.append('\t<img class="logo" alt="logo" src="/logo.png" />')
		body.append('</a>\n')
		body.append('<h1 class="titlebar">%s</h1>\n' % pageTitle)

		# Generate navigation bar
		body.append('<div class="navbar">\n')
		body.append('\t<div class="navgroup">')
		body.append('\t\t<div class="navhome">')
		body.append('\t\t\t<a href="%s">%s</a>' %\
			    (self.__makePageUrl(None, None),
			     self.db.getString("home")))
		body.append('\t\t</div>')
		body.append('\t</div>\n')
		navGroups = self.db.getGroupNames()
		navGroups.sort(key=lambda (name, label): label)
		for (navgroupname, navgrouplabel) in navGroups:
			body.append('\t<div class="navgroup">')
			if navgrouplabel:
				body.append('\t\t<div class="navhead">%s</div>' % navgrouplabel)
			navPages = self.db.getPageNames(navgroupname)
			navPages.sort(key=lambda (name, label): label)
			for (navpagename, navpagelabel) in navPages:
				body.append('\t\t<div class="navelem">')
				url = self.__makePageUrl(navgroupname, navpagename)
				body.append('\t\t\t<a href="%s">%s</a>' % (url, navpagelabel))
				body.append('\t\t</div>')
			body.append('\t</div>\n')
		body.append('</div>\n')

		body.append('<div class="main">\n') # Main body start

		body.append('<!-- BEGIN: main part -->')
		body.append(pageData)
		body.append('<!-- END: main part -->\n')

		# Last-modified date
		body.append('\t<div class="modifystamp">')
		body.append('\t\tUpdated: %s' % stamp.strftime("%A %d %B %Y %H:%M"))
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
		body.append('\t<div class="checker">')
		body.append('\t\t<a href="http://validator.w3.org/check?uri=referer">%s</a> /' %\
			    self.db.getString("checker-xhtml"))
		body.append('\t\t<a href="http://jigsaw.w3.org/css-validator/check/referer">%s</a>' %\
			    self.db.getString("checker-css"))
		body.append('\t</div>\n')

		body.append('</div>\n') # Main body end

		return "\n".join(body)

	def __expandOneMacro(self, match, recurseLevel):
		def expandParam(match):
			pnumber = int(match.group(1))
			try:
				assert(pnumber >= 1)
				return parameters[pnumber - 1]
			except IndexError, AssertionError:
				return "" # Param not given
		def expandCond(match):
			if match.group(1).strip(): # Check cond. for non-empty
				return match.group(2) # THEN-branch
			try:
				return match.group(3) # ELSE-branch
			except IndexError:
				return "" # No ELSE branch. Return empty string.
		def sanitize(match):
			validChars = "abcdefghijklmnopqrstuvwxyz1234567890"
			string = match.group(1).lower()
			string = "".join(( c if c in validChars else '_' for c in string ))
			string = re.sub(r'_+', '_', string).strip('_')
			return string
		macroname = match.group(1)
		parameters = [ p.strip() for p in match.group(2).split(",") ]
		# Get the raw macro value
		macrovalue = self.db.getMacro(macroname)
		if not macrovalue:
			return "\n<!-- WARNING: INVALID MACRO USED: %s -->\n" %\
				match.group(0)
		# Expand the parameters
		macrovalue = self.macro_param_re.sub(expandParam, macrovalue)
		# Expand variables
		vars = {
			"$GROUP"	: self.currentGroupname,
			"$PAGE"		: self.currentPagename,
		}
		for var in vars.keys():
			macrovalue = macrovalue.replace(var, vars[var])
		# Sanitize strings
		macrovalue = self.macro_strsan_re.sub(sanitize, macrovalue)
		# Expand the conditionals
		while True:
			(macrovalue, n) = self.macro_ifcond_re.subn(expandCond, macrovalue)
			if not n:
				break
		# Expand recursive macros
		macrovalue = self.__expandMacros(macrovalue, recurseLevel + 1)
		return macrovalue

	def __expandMacros(self, data, recurseLevel=0):
		if recurseLevel > 8:
			raise CMSException(500, "Exceed macro recurse depth")
		return self.macro_re.sub(lambda m: self.__expandOneMacro(m, recurseLevel),
					 data)

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
			if pagename.endswith(".html"):
				pagename = pagename[:-5]
			if not groupname or not pagename:
				raise CMSException(404)
		return (groupname, pagename)

	@cache_region("image", "thumbnail")
	def __getImageThumbnail(self, imagename, query):
		try:
			width = int(query["w"][0])
			height = int(query["h"][0])
		except (KeyError, IndexError, ValueError), e:
			raise CMSException(400)
		try:
			imagename = validateName(imagename)
			img = Image.open(mkpath(self.imagesPath, imagename))
			img.thumbnail((width, height), Image.ANTIALIAS)
			output = StringIO()
			img.save(output, "JPEG")
			data = output.getvalue()
		except (IOError), e:
			raise CMSException(404)
		return (data, "image/jpeg")

	@cache_region("html", "page")
	def __getHtmlPage(self, groupname, pagename, cssPath):
		(pageTitle, pageData, stamp) = self.db.getPage(groupname, pagename)
		pageData = self.__expandMacros(pageData)
		data = [self.__genHtmlHeader(pageTitle, cssPath)]
		data.append(self.__genHtmlBody(groupname, pagename,
					       pageTitle, pageData, stamp))
		data.append(self.__genHtmlFooter())
		return ("".join(data), "text/html")

	def __generate(self, path, cssPath, query):
		(groupname, pagename) = self.__parsePagePath(path)
		self.currentGroupname = groupname
		self.currentPagename = pagename
		if groupname == "__thumbs":
			return self.__getImageThumbnail(pagename, query)
		return self.__getHtmlPage(groupname, pagename, cssPath)

	def get(self, path, query={}):
		cssPath = self.cssPath
		try:
			if stringBool(query["print"][0]):
				cssPath = self.cssPrintPath
		except (KeyError, IndexError), e: pass
		return self.__generate(path, cssPath, query)

	def post(self, path, query={}):
		raise CMSException(405)

	def getErrorPage(self, cmsExcept):
		html = [self.__genHtmlHeader("Error - %s" % cmsExcept.httpStatus,
					     self.cssPath)]
		html.append('<h1>An error occurred</h1>')
		html.append('<p>We\'re sorry for the inconvenience. ')
		html.append('The page could not be accessed, because:</p>')
		html.append('<p style="font-size: xx-large;">')
		html.append(cmsExcept.httpStatus)
		if cmsExcept.message:
			html.append('<br />')
			html.append(cmsExcept.message)
		html.append('</p>')
		html.append('<p>You may visit the <a href="%s">main page</a>' %\
			    self.__makePageUrl(None, None))
		html.append('and navigate manually to your desired target page.</p>')
		html.append(self.__genHtmlFooter())

		return "\n".join(html)

if __name__ == "__main__":
	if len(sys.argv) != 2:
		print "Usage: %s /path/to/page" % sys.argv[0]
		sys.exit(1)
	path = sys.argv[1]
	try:
		cms = CMS("./db")
		sys.stdout.write(cms.get(path))
	except (CMSException), e:
		sys.stdout.write(cms.getErrorPage(e))
	cms.shutdown()

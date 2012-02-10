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
			"expire"	: 86400,
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

def validateName(name):
	# Validate a name string for use as safe path component
	# Raises CMSException on failure.
	if name.startswith('.'):
		# No ".", ".." and hidden files.
		raise CMSException(404)
	validChars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-_."
	if [ c for c in name if c not in validChars ]:
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

class CMS:
	# Macro call: @name(param1, param2, ...)
	macro_re = re.compile(r'@(\w+)\(([^\)]*)\)', re.DOTALL)
	# Macro parameter expansion: $1, $2, $3...
	macro_param_re = re.compile(r'\$(\d+)', re.DOTALL)
	# Macro if-conditional: $(if COND,THEN,ELSE)
	macro_ifcond_re = re.compile(r'\$\(if\s+([^,\)]*),([^,\)]*)(?:,([^,\)]*))?\)', re.DOTALL)
	# Macro string sanitize: $(sanitize STRING)
	macro_strsan_re = re.compile(r'\$\(sanitize\s+([^,\)]*)\)', re.DOTALL)
	# Content comment <!--- comment --->
	comment_re = re.compile(r'<!---(.*)--->', re.DOTALL)

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
				prio = 999
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

	def __expandOneMacro(self, match, recurseLevel):
		def expandParam(match):
			pnumber = int(match.group(1), 10)
			if pnumber >= 1 and pnumber <= len(parameters):
				return parameters[pnumber - 1]
			if pnumber == 0:
				return macroname
			return "" # Param not given or invalid
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
		exvars = (
			("$GROUP"	, self.currentGroupname),
			("$PAGE"	, self.currentPagename),
		)
		for var, value in exvars:
			macrovalue = macrovalue.replace(var, value)
		# Sanitize strings
		macrovalue = self.macro_strsan_re.sub(sanitize, macrovalue)
		# Expand the conditionals
		n = 1
		while n:
			macrovalue, n = self.macro_ifcond_re.subn(expandCond, macrovalue)
		# Expand recursive macros
		macrovalue = self.__expandMacros(macrovalue, recurseLevel + 1)
		return macrovalue

	def __expandMacros(self, data, recurseLevel=0):
		if recurseLevel > 8:
			raise CMSException(500, "Exceed macro recurse depth")
		return self.macro_re.sub(lambda m: self.__expandOneMacro(m, recurseLevel),
					 data)

	def __handleContentComments(self, data):
		return self.comment_re.sub("", data)

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
			imagename = validateName(imagename)
			img = Image.open(mkpath(self.imagesPath, imagename))
			img.thumbnail((width, height), Image.ANTIALIAS)
			output = StringIO()
			img.save(output, "JPEG")
			data = output.getvalue()
		except (IOError), e:
			raise CMSException(404)
		return (data, "image/jpeg")

#	@cache_region("html", "page")
	def __getHtmlPage(self, groupname, pagename, cssPath):
		(pageTitle, pageData, stamp) = self.db.getPage(groupname, pagename)
		if not pageData:
			raise CMSException(404)
		pageData = self.__expandMacros(pageData)
		pageData = self.__handleContentComments(pageData)
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
		html.append('<p>You may visit the <a href="%s" style="font-size: xx-large;">main page</a>' %\
			    self.__makePageUrl(None, None))
		html.append('and navigate manually to your desired target page.</p>')
		html.append(self.__genHtmlFooter())

		return ("\n".join(html), "text/html")

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

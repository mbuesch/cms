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

from cms.db import *
from cms.exception import *
from cms.formfields import *
from cms.pageident import *
from cms.query import *
from cms.resolver import * #+cimport
from cms.sitemap import *
from cms.util import * #+cimport

import functools
import PIL.Image as Image
import urllib.parse

__all__ = [
	"CMS",
]

class CMS(object):
	# Main CMS entry point.

	__rootPageIdent = CMSPageIdent()

	def __init__(self,
		     wwwPath,
		     imagesDir="/images",
		     domain="example.com",
		     urlBase="/cms",
		     cssUrlPath="/cms.css",
		     debug=False):
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

		self.db = CMSDatabase()
		self.resolver = CMSStatementResolver(self)

	def shutdown(self):
		pass

	def __genHtmlHeader(self, title, additional = ""):
		date = datetime.now(dt_timezone.utc).isoformat()
		interpreter = "Python" #@nocy
#		interpreter = "Cython" #@cy
		sitemap = self.urlBase + "/__sitemap.xml"
		additional = "\n\t".join(additional.splitlines())

		return f"""<?xml version="1.0" encoding="UTF-8" ?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" lang="en" xml:lang="en">
<head>
	<!--
		Generated by "cms.py - simple CMS script"
		https://bues.ch/cgit/cms.git
	-->
	<meta name="generator" content="WSGI/{interpreter} CMS" />
	<meta name="date" content="{date}" />
	<meta name="robots" content="all" />
	<title>{title}</title>
	<link rel="stylesheet" href="{self.cssUrlPath}" type="text/css" />
	<link rel="sitemap" type="application/xml" title="Sitemap" href="{sitemap}" />
	{additional or ''}
</head>
<body>
"""

	def __genHtmlFooter(self):
		footer = """
</body>
</html>
"""
		return footer

	def _genNavElem(self, body, basePageIdent, activePageIdent, indent=0):
		if self.db.getNavStop(basePageIdent):
			return
		subPages = self.db.getSubPages(basePageIdent)
		if not subPages:
			return
		tabs = '\t' + '\t' * indent
		if indent > 0:
			body.append('%s<div class="navelems">' % tabs)
		for pagename, pagelabel, pageprio in subPages:
			if not pagelabel:
				continue
			body.append('%s\t<div class="%s"> '
				    '<!-- %d -->' % (
				    tabs,
				    "navelem" if indent > 0 else "navgroup",
				    pageprio))
			if indent <= 0:
				body.append('%s\t\t<div class="navhead">' %\
					    tabs)
			subPageIdent = CMSPageIdent(basePageIdent + [pagename])
			isActive = (activePageIdent is not None and
				    activePageIdent.startswith(subPageIdent))
			if isActive:
				body.append('%s\t\t<div class="navactive">' %\
					    tabs)
			body.append('%s\t\t<a href="%s">%s</a>' %\
				    (tabs,
				     subPageIdent.getUrl(urlBase=self.urlBase),
				     pagelabel))
			if isActive:
				body.append('%s\t\t</div> '
					    '<!-- class="navactive" -->' %\
					    tabs)
			if indent <= 0:
				body.append('%s\t\t</div>' % tabs)

			self._genNavElem(body, subPageIdent,
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
			    (self.__rootPageIdent.getUrl(urlBase=self.urlBase),
			     self.db.getString("home")))
		if rootActive:
			body.append('\t\t</div> <!-- class="navactive" -->')
		body.append('\t\t</div>')
		self._genNavElem(body, self.__rootPageIdent, pageIdent)
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

	def __getSiteMap(self, query, protocol):
		sitemap = CMSSiteMap(self.db, self.domain, self.urlBase)
		data = sitemap.getSiteMap(self.__rootPageIdent, protocol)
		try:
			return (data.encode("UTF-8", "strict"),
				"text/xml; charset=UTF-8")
		except UnicodeError as e:
			raise CMSException(500, "Unicode encode error")

	@functools.lru_cache(maxsize=2**8)
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
			3 : getattr(Image, "LANCZOS", getattr(Image, "ANTIALIAS", Image.BICUBIC)),
		}
		try:
			qual = qualities[qual]
		except (KeyError) as e:
			qual = qualities[1]
		try:
			imgPath = fs.mkpath(self.wwwPath,
					    self.imagesDir,
					    CMSPageIdent.validateSafePathComponent(imagename))
			with open(imgPath.encode("UTF-8", "strict"), "rb") as fd:
				with Image.open(fd) as img:
					img.thumbnail((width, height), qual)
					with img.convert("RGB") as cimg:
						output = BytesIO()
						cimg.save(output, "JPEG")
						data = output.getvalue()
		except (IOError, UnicodeError) as e:
			raise CMSException(404)
		return data, "image/jpeg"

	def __getHtmlPage(self, pageIdent, query, protocol):
		pageTitle, pageData, stamp = self.db.getPage(pageIdent)
		if not pageData:
			raise CMSException(404)

		resolverVariables = {
			"PROTOCOL"	: lambda r, n: protocol,
			"PAGEIDENT"	: lambda r, n: pageIdent.getUrl(),
			"CMS_PAGEIDENT"	: lambda r, n: pageIdent.getUrl(urlBase=self.urlBase),
			"GROUP"		: lambda r, n: pageIdent.get(0),
			"PAGE"		: lambda r, n: pageIdent.get(1),
		}
		resolve = self.resolver.resolve
		for k, v in query.items():
			k = k.upper()
			resolverVariables["Q_" + k] = self.resolver.escape(htmlEscape(v))
			resolverVariables["QRAW_" + k] = self.resolver.escape(v)
		pageTitle = resolve(pageTitle, resolverVariables, pageIdent)
		resolverVariables["TITLE"] = lambda r, n: pageTitle
		pageData = resolve(pageData, resolverVariables, pageIdent)
		extraHeaders = resolve(self.db.getHeaders(pageIdent),
				       resolverVariables, pageIdent)

		data = [self.__genHtmlHeader(pageTitle, extraHeaders)]
		data.append(self.__genHtmlBody(pageIdent,
					       pageTitle, pageData,
					       protocol, stamp))
		data.append(self.__genHtmlFooter())
		try:
			return ("".join(data).encode("UTF-8", "strict"),
				"application/xhtml+xml; charset=UTF-8")
		except UnicodeError as e:
			raise CMSException(500, "Unicode encode error")

	def __get(self, path, query, protocol):
		pageIdent = CMSPageIdent.parse(path)
		firstIdent = pageIdent.get(0, allowSysNames=True)
		if firstIdent == "__thumbs":
			return self.__getImageThumbnail(pageIdent.get(1), query, protocol)
		elif firstIdent in ("__sitemap", "__sitemap.xml"):
			return self.__getSiteMap(query, protocol)
		return self.__getHtmlPage(pageIdent, query, protocol)

	def get(self, path, query={}, protocol="http"):
		query = CMSQuery(query)
		return self.__get(path, query, protocol)

	def __post(self, path, query, body, bodyType, protocol):
		pageIdent = CMSPageIdent.parse(path)
		formFields = CMSFormFields(body, bodyType)
		try:
			ret = self.db.runPostHandler(pageIdent, formFields, query)
		except CMSException as e:
			raise e
		except Exception as e:
			msg = ""
			if self.debug:
				msg = " " + str(e)
			msg = msg.encode("UTF-8", "ignore")
			return (b"Failed to run POST handler." + msg,
				"text/plain")
		if ret is None:
			return self.__get(path, query, protocol)
		assert isinstance(ret, tuple) and len(ret) == 2, "post() return is not 2-tuple."
		assert isinstance(ret[0], (bytes, bytearray)), "post()[0] is not bytes."
		assert isinstance(ret[1], str), "post()[1] is not str."
		return ret

	def post(self, path, query={},
		 body=b"", bodyType="text/plain",
		 protocol="http"):
		query = CMSQuery(query)
		return self.__post(path, query, body, bodyType, protocol)

	def __doGetErrorPage(self, cmsExcept, protocol):
		resolverVariables = {
			"PROTOCOL"		: lambda r, n: protocol,
			"GROUP"			: lambda r, n: "_nogroup_",
			"PAGE"			: lambda r, n: "_nopage_",
			"HTTP_STATUS"		: lambda r, n: cmsExcept.httpStatus,
			"HTTP_STATUS_CODE"	: lambda r, n: str(cmsExcept.httpStatusCode),
			"ERROR_MESSAGE"		: lambda r, n: self.resolver.escape(htmlEscape(cmsExcept.message)),
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
		return "".join(data), "application/xhtml+xml; charset=UTF-8", httpHeaders

	def getErrorPage(self, cmsExcept, protocol="http"):
		try:
			data, mime, headers = self.__doGetErrorPage(cmsExcept, protocol)
		except (CMSException) as e:
			data = "Error in exception handler: %s %s" % \
				(e.httpStatus, e.message)
			mime, headers = "text/plain; charset=UTF-8", ()
		try:
			return data.encode("UTF-8", "strict"), mime, headers
		except UnicodeError as e:
			# Whoops. All is lost.
			raise CMSException(500, "Unicode encode error")

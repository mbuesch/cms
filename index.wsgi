#!/usr/bin/python
#
#   CMS WSGI wrapper
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
import atexit
try:
	from urlparse import parse_qs
except ImportError:
	from cgi import parse_qs
try:
	sys.path.append("/var/cms") # workaround for old WSGI
	from cms import *
except ImportError:
	raise Exception("Failed to import cms.py. Wrong WSGIPythonPath?")

cms = None

def __initCMS(environ):
	global cms
	if cms:
		return # Already initialized
	try:
		domain = environ["cms.domain"]
		cmsBase = environ["cms.cmsBase"]
		wwwBase = environ["cms.wwwBase"]
	except (KeyError), e:
		raise Exception("WSGI environment %s not set" % str(e))
	debug = False
	try:
		debug = stringBool(environ["cms.debug"])
	except (KeyError), e:
		pass
	# Initialize the CMS module
	cms = CMS(dbPath = cmsBase + "/db",
		  wwwPath = wwwBase,
		  domain = domain,
		  debug = debug)
	atexit.register(cms.shutdown)

def application(environ, start_response):
	__initCMS(environ)
	if cms.debug:
		startStamp = datetime.now()
	status = "200 OK"
	additional_headers = []
	try:
		method = environ["REQUEST_METHOD"].upper()
		path = environ["PATH_INFO"]
		query = parse_qs(environ["QUERY_STRING"])
		if method == "GET":
			response_body, response_mime = cms.get(path, query)
		elif method == "POST":
			response_body, response_mime = cms.post(path, query)
		else:
			response_body, response_mime =\
				"INVALID REQUEST_METHOD", "text/plain"
	except (CMSException), e:
		status = e.httpStatus
		response_body, response_mime, additional_headers = cms.getErrorPage(e)
	if cms.debug and response_mime.lower() == "text/html":
		delta = datetime.now() - startStamp
		sec = float(delta.seconds) + float(delta.microseconds) / 1000000
		response_body += "\n<!-- generated in %.3f seconds -->" % sec
	response_headers = [ ('Content-Type', response_mime),
			     ('Content-Length', str(len(response_body))) ]
	response_headers.extend(additional_headers)
	start_response(status, response_headers)
	return (response_body,)

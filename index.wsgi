#!/usr/bin/python
# -*- coding: utf-8 -*-
#
#   CMS WSGI wrapper
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
import atexit
try:
	from urllib.parse import parse_qs
except ImportError:
	from cgi import parse_qs
try:
	sys.path.append("/var/cms") # workaround for old WSGI
	from cms import *
except ImportError as e:
	raise Exception("Failed to import cms.py. Wrong python-path in WSGIDaemonProcess?:\n" + str(e))

cms = None
maxPostContentLength = 0

def __initCMS(environ):
	global cms
	global maxPostContentLength
	if cms:
		return # Already initialized
	try:
		domain = environ["cms.domain"]
		cmsBase = environ["cms.cmsBase"]
		wwwBase = environ["cms.wwwBase"]
	except KeyError as e:
		raise Exception("WSGI environment %s not set" % str(e))
	debug = False
	try:
		debug = stringBool(environ["cms.debug"])
	except KeyError as e:
		pass
	try:
		maxPostContentLength = int(environ["cms.maxPostContentLength"], 10)
	except (KeyError, ValueError) as e:
		pass
	# Initialize the CMS module
	cms = CMS(dbPath = cmsBase + "/db",
		  wwwPath = wwwBase,
		  domain = domain,
		  debug = debug)
	atexit.register(cms.shutdown)

def __recvBody(environ):
	try:
		body_len = int(environ["CONTENT_LENGTH"], 10)
	except (ValueError, KeyError) as e:
		body_len = 0
	try:
		body_type = environ["CONTENT_TYPE"]
	except KeyError as e:
		body_type = "text/plain"
	if body_len < 0 or \
	   (maxPostContentLength >= 0 and\
	    body_len > maxPostContentLength):
		body = body_type = None
	else:
		body = environ["wsgi.input"].read(body_len)
	return body, body_type

def application(environ, start_response):
	__initCMS(environ)
	if cms.debug:
		startStamp = datetime.now()

	status = "200 OK"
	additional_headers = []

	method = environ["REQUEST_METHOD"].upper()
	path = environ["PATH_INFO"]
	query = parse_qs(environ["QUERY_STRING"])
	protocol = environ["wsgi.url_scheme"].lower()
	try:
		if method == "GET":
			response_body, response_mime = cms.get(path, query, protocol)
		elif method == "POST":
			body, body_type = __recvBody(environ)
			if body is None:
				response_body, response_mime, status = (
					b"POSTed data is too long\n",
					"text/plain",
					"405 Method Not Allowed"
				)
			else:
				response_body, response_mime = cms.post(path, query,
									body, body_type,
									protocol)
		else:
			response_body, response_mime, status = (
				b"INVALID REQUEST_METHOD\n",
				"text/plain",
				"405 Method Not Allowed"
			)
	except (CMSException) as e:
		status = e.httpStatus
		response_body, response_mime, additional_headers = cms.getErrorPage(e, protocol)
	if cms.debug and response_mime.startswith("text/html"):
		delta = datetime.now() - startStamp
		sec = float(delta.seconds) + float(delta.microseconds) / 1000000
		response_body += ("\n<!-- generated in %.3f seconds -->" % sec).encode("UTF-8")
	response_headers = [ ('Content-Type', response_mime),
			     ('Content-Length', str(len(response_body))) ]
	response_headers.extend(additional_headers)
	start_response(status, response_headers)
	return (response_body,)

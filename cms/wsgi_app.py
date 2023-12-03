# -*- coding: utf-8 -*-
#
#   CMS WSGI wrapper
#
#   Copyright (C) 2011-2023 Michael Buesch <m@bues.ch>
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
from urllib.parse import parse_qs

from cms import *
from cms.util import * #+cimport

__all__ = [
	"application",
]

cms = None
maxPostContentLength = 0

def __initCMS(environ):
	global cms
	global maxPostContentLength

	domain = environ.get("cms.domain", None)
	cmsBase = environ.get("cms.cmsBase", None)
	wwwBase = environ.get("cms.wwwBase", None)
	if domain is None or cmsBase is None or wwwBase is None:
		raise Exception("WSGI environment cms.domain, cms.cmsBase "
				"or cms.wwwBase not set.")
	debug = stringBool(environ.get("cms.debug", "0"), False)
	try:
		maxPostContentLength = int(environ.get("cms.maxPostContentLength", "0"))
	except ValueError as e:
		maxPostContentLength = 0
	# Initialize the CMS module
	cms = CMS(dbPath=(cmsBase + "/db"),
		  wwwPath=wwwBase,
		  domain=domain,
		  debug=debug)
	atexit.register(cms.shutdown)

def __recvBody(environ):
	global maxPostContentLength

	try:
		body_len = int(environ.get("CONTENT_LENGTH", "0"))
	except ValueError as e:
		body_len = 0
	body_type = environ.get("CONTENT_TYPE", "text/plain")
	if (body_len >= 0 and
	    (body_len <= maxPostContentLength or maxPostContentLength < 0)):
		wsgi_input = environ.get("wsgi.input", None)
		if wsgi_input is None:
			raise Exception("WSGI environment 'wsgi.input' not set.")
		body = wsgi_input.read(body_len)
	else:
		body = body_type = None
	return body, body_type

def application(environ, start_response):
	global cms

	if cms is None:
		__initCMS(environ)
	if cms.debug:
		import time
		startStamp = time.monotonic()

	status = "200 OK"
	additional_headers = []

	method = environ.get("REQUEST_METHOD", "").upper()
	path = environ.get("PATH_INFO", "")
	query = parse_qs(environ.get("QUERY_STRING", ""))
	protocol = environ.get("wsgi.url_scheme", "http").lower()
	try:
		if method == "GET" or method == "HEAD":
			response_body, response_mime = cms.get(path, query, protocol)
			if method == "HEAD":
				response_body = b""
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
	if cms.debug and "html" in response_mime:
		delta = time.monotonic() - startStamp
		ms = delta * 1e3
		response_body += ("\n<!-- generated in %.3f ms -->" % ms).encode("UTF-8", "ignore")
	response_headers = [ ('Content-Type', response_mime),
			     ('Content-Length', str(len(response_body))) ]
	response_headers.extend(additional_headers)
	start_response(status, response_headers)
	return (response_body,)

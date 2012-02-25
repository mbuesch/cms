#!/usr/bin/python

# Website domain
domain		= "example.com"
# Path to "cms.py" and CMS "db" directory
cmsBase		= "/var/cms"
# Path to static content files
wwwBase		= "/var/www"


import sys
sys.path.insert(0, cmsBase) # Path to cms.py
import atexit
from cms import *
try:
	from urlparse import parse_qs
except ImportError:
	from cgi import parse_qs

def application(environ, start_response):
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
				"INVALID REQUEST_METHOD", "text/html"
	except (CMSException), e:
		status = e.httpStatus
		response_body, response_mime, additional_headers = cms.getErrorPage(e)
	if response_mime.lower() == "text/html":
		delta = datetime.now() - startStamp
		sec = float(delta.seconds) + float(delta.microseconds) / 1000000
		response_body += "\n<!-- generated in %.3f seconds -->" % sec
	response_headers = [ ('Content-Type', response_mime),
			     ('Content-Length', str(len(response_body))) ]
	response_headers.extend(additional_headers)
	start_response(status, response_headers)
	return (response_body,)

cms = CMS(dbPath = cmsBase + "/db",
	  wwwPath = wwwBase,
	  domain = domain)
atexit.register(cms.shutdown)

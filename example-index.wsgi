#!/usr/bin/python

# Path to "cms.py" and CMS "db" directory
cmsBase		= "/home/www/cms"
# Path to static content files
wwwBase		= "/var/www/"
# Website domain
domain		= "example.com"
# URL rewriting rules
rewriteURLs = {
#	"FROM-URL"	: "TO-URL",
}


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
	try:
		method = environ["REQUEST_METHOD"].upper()
		path = environ["PATH_INFO"]
		try:
			path = rewriteURLs[path]
		except KeyError: pass
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
		response_body, response_mime = cms.getErrorPage(e)
	if response_mime.lower() == "text/html":
		delta = datetime.now() - startStamp
		sec = float(delta.seconds) + float(delta.microseconds) / 1000000
		response_body += "\n<!-- generated in %.3f seconds -->" % sec
	start_response(status,
		       [ ('Content-Type', response_mime),
			 ('Content-Length', str(len(response_body))) ])
	return (response_body,)

cms = CMS(dbPath = cmsBase + "/db",
	  imagesPath = wwwBase + "/images",
	  domain = domain)
atexit.register(cms.shutdown)

# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2021 Michael Buesch <m@bues.ch>
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
from cms.pageident import *
from cms.util import * #+cimport

from xml.sax import saxutils

__all__ = [
	"CMSSiteMap",
]

class CMSSiteMap(object):
	"""Site map generator.
	Specification: https://www.sitemaps.org/protocol.html
	"""

	BASE_INDENT	= 1
	INDENT		= "  "
	MORE_ESCAPES	= {
		"'" : "&apos;",
		'"' : "&quot;",
	}

	def __init__(self, db, domain, urlBase):
		self.__db = db
		self.__domain = domain
		self.__urlBase = urlBase

	@classmethod
	def __xmlQuote(cls, string):
		return saxutils.escape(string, cls.MORE_ESCAPES)

	@classmethod
	def __oneElem(cls, ind, url, lastmod=None, changefreq=None, prio=None):
		ret = [ f'{ind}<url>' ]
		url = cls.__xmlQuote(url)
		ret.append(f'{ind}{cls.INDENT}<loc>{url}</loc>')
		if lastmod:
			lastmod = cls.__xmlQuote(lastmod)
			ret.append(f'{ind}{cls.INDENT}<lastmod>{lastmod}</lastmod>')
		if changefreq:
			changefreq = cls.__xmlQuote(changefreq)
			ret.append(f'{ind}{cls.INDENT}<changefreq>{changefreq}</changefreq>')
		if prio:
			prio = cls.__xmlQuote(prio)
			ret.append(f'{ind}{cls.INDENT}<priority>{prio}</priority>')
		ret.append(f'{ind}</url>')
		return ret

	def __getUrlElems(self, pageIdent, protocol, indent=BASE_INDENT):
		if self.__db.getNavStop(pageIdent):
			return

		ind = self.INDENT * indent
		if indent <= self.BASE_INDENT + 1:
			pageSuffix = "/" # Groups.
		else:
			pageSuffix = ".html" # Pages and sub groups.
		url = pageIdent.getUrl(protocol=protocol,
				       domain=self.__domain,
				       urlBase=self.__urlBase,
				       pageSuffix=pageSuffix)
		lastmod = self.__db.getPageStamp(pageIdent).strftime("%Y-%m-%dT%H:%M:%SZ")
		if indent == self.BASE_INDENT + 1:
			prio = "0.3" # Main groups.
		else:
			prio = "0.7" # Pages, main page and sub groups.
		yield self.__oneElem(ind=ind,
				     url=url,
				     lastmod=lastmod,
				     prio=prio)

		subPages = self.__db.getSubPages(pageIdent)
		if subPages:
			for pagename, pagelabel, pageprio in subPages:
				subPageIdent = CMSPageIdent(pageIdent + [pagename])
				yield from self.__getUrlElems(subPageIdent,
							      protocol,
							      indent + 1)

	def __getUserUrlElems(self, protocol):
		userSiteMap = self.__db.getString("site-map")
		if not userSiteMap:
			return
		for line in userSiteMap.splitlines():
			line = line.strip()
			if not line or line.startswith("#"):
				continue
			lineItems = line.split()
			if len(lineItems) == 1:
				url, prio, changefreq = lineItems[0], "0.7", "always"
			elif len(lineItems) == 2:
				url, prio, changefreq = lineItems[0], lineItems[1], "always"
			elif len(lineItems) == 3:
				url, prio, changefreq = lineItems[0], lineItems[1], lineItems[2]
			else:
				raise CMSException(500, "site-map: Invalid line format.")
			try:
				float(prio)
			except Exception:
				raise CMSException(500, "site-map: Invalid priority value.")
			if changefreq not in ("always", "hourly", "daily", "weekly",
					      "monthly", "yearly", "never",):
				raise CMSException(500, "site-map: Invalid changefreq value.")
			url = f'{protocol}://{self.__domain}/{url}'
			yield self.__oneElem(ind=self.INDENT,
					     url=url,
					     changefreq=changefreq,
					     prio=prio)

	def getSiteMap(self, rootPageIdent, protocol):
		ret = [ '<?xml version="1.0" encoding="UTF-8"?>' ]
		ret.append('<urlset xmlns="https://www.sitemaps.org/schemas/sitemap/0.9" '
			   'xmlns:xsi="https://www.w3.org/2001/XMLSchema-instance" '
			   'xsi:schemaLocation="https://www.sitemaps.org/schemas/sitemap/0.9 '
			   'https://www.sitemaps.org/schemas/sitemap/0.9/sitemap.xsd">')
		for urlElemLines in self.__getUrlElems(rootPageIdent, protocol):
			ret.extend(urlElemLines)
		for urlElemLines in self.__getUserUrlElems(protocol):
			ret.extend(urlElemLines)
		ret.append('</urlset>')
		return "\n".join(ret)

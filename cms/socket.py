# -*- coding: utf-8 -*-
#
#   cms.py - simple WSGI/Python based CMS script
#
#   Copyright (C) 2011-2024 Michael Buesch <m@bues.ch>
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

from cms.exception import *

import socket
import sys

MSG_HDR_LEN = 8

MAGIC_DB = 0x8F5755D6
ID_DB_GETPAGE = 0
ID_DB_GETHEADERS = 1
ID_DB_GETSUBPAGES = 2
ID_DB_GETMACRO = 3
ID_DB_GETSTRING = 4
ID_DB_PAGE = 5
ID_DB_HEADERS = 6
ID_DB_SUBPAGES = 7
ID_DB_MACRO = 8
ID_DB_STRING = 9

MAGIC_POST = 0x6ADCB73F
ID_POST_RUNPOSTHANDLER = 0
ID_POST_POSTHANDLERRESULT = 1

def pack_u8(value):
	return (value & 0xFF).to_bytes(1, sys.byteorder)

def pack_u32(value):
	return (value & 0xFFFFFFFF).to_bytes(4, sys.byteorder)

def pack_u64(value):
	return (value & 0xFFFFFFFFFFFFFFFF).to_bytes(8, sys.byteorder)

def pack_bool(value):
	return pack_u8(1 if value else 0)

def pack_bytes(buf):
	ret = bytearray(pack_u64(len(buf)))
	ret += buf
	return ret

def pack_str(string):
	try:
		return pack_bytes(string.encode("UTF-8", "strict"))
	except UnicodeError:
		raise ValueError("pack_str: Invalid string encoding.")

def pack_hashmap_str_bytes(items):
	ret = bytearray(pack_u64(len(items)))
	for item in items:
		ret += pack_str(items[0])
		ret += pack_bytes(items[1])
	return ret

def pack_message(payload):
	ret = bytearray(pack_u32(MAGIC_DB))
	ret += pack_u32(len(payload))
	ret += payload
	return ret

def unpack_u8(buf, i):
	return int.from_bytes(buf[i:i+1], sys.byteorder), i + 1

def unpack_u32(buf, i):
	return int.from_bytes(buf[i:i+4], sys.byteorder), i + 4

def unpack_u64(buf, i):
	return int.from_bytes(buf[i:i+8], sys.byteorder), i + 8

def unpack_bool(buf, i):
	v, i = unpack_u8(buf, i)
	return v != 0, i

def unpack_bytes(buf, i):
	count, i = unpack_u64(buf, i)
	return buf[i:i+count], i + count

def unpack_str(buf, i):
	v, i = unpack_bytes(buf, i)
	try:
		return v.decode("UTF-8", "strict"), i
	except UnicodeError:
		raise ValueError("unpack_str: Invalid string encoding.")

def unpack_hashmap_str_bytes(buf, i):
	pass#TODO

def unpack_header(buf, magic_expected):
	if len(buf) < MSG_HDR_LEN:
		raise ValueError("unpack_header: Too short.")
	magic, i = unpack_u32(buf, 0)
	if magic != magic_expected:
		raise ValueError("unpack_header: Invalid magic.")
	plLen, i = unpack_u32(buf, i)
	return plLen

class MsgGetPage:
	def __init__(
		self,
		path,
		get_title=False,
		get_data=False,
		get_stamp=False,
		get_prio=False,
		get_redirect=False,
		get_nav_stop=False,
		get_nav_label=False,
	):
		self.path = path
		self.get_title = get_title
		self.get_data = get_data
		self.get_stamp = get_stamp
		self.get_prio = get_prio
		self.get_redirect = get_redirect
		self.get_nav_stop = get_nav_stop
		self.get_nav_label = get_nav_label

	def pack(self):
		payload = bytearray(pack_u32(ID_DB_GETPAGE))
		payload += pack_str(self.path)
		payload += pack_bool(self.get_title)
		payload += pack_bool(self.get_data)
		payload += pack_bool(self.get_stamp)
		payload += pack_bool(self.get_prio)
		payload += pack_bool(self.get_redirect)
		payload += pack_bool(self.get_nav_stop)
		payload += pack_bool(self.get_nav_label)
		return pack_message(payload)

class MsgGetHeaders:
	def __init__(self, path):
		self.path = path

	def pack(self):
		payload = bytearray(pack_u32(ID_DB_GETHEADERS))
		payload += pack_str(self.path)
		return pack_message(payload)

class MsgGetSubPages:
	def __init__(self, path):
		self.path = path

	def pack(self):
		payload = bytearray(pack_u32(ID_DB_GETSUBPAGES))
		payload += pack_str(self.path)
		return pack_message(payload)

class MsgGetMacro:
	def __init__(self, parent, name):
		self.parent = parent
		self.name = name

	def pack(self):
		payload = bytearray(pack_u32(ID_DB_GETMACRO))
		payload += pack_str(self.parent)
		payload += pack_str(self.name)
		return pack_message(payload)

class MsgGetString:
	def __init__(self, name):
		self.name = name

	def pack(self):
		payload = bytearray(pack_u32(ID_DB_GETSTRING))
		payload += pack_str(self.name)
		return pack_message(payload)

class MsgPage:
	def __init__(
		self,
		title=None,
		data=None,
		stamp=None,
		prio=None,
		redirect=None,
		nav_stop=None,
		nav_label=None,
	):
		self.title = title
		self.data = data
		self.stamp = stamp
		self.prio = prio
		self.redirect = redirect
		self.nav_stop = nav_stop
		self.nav_label = nav_label

	@classmethod
	def unpack(cls, buf, i):
		self = cls()

		have_title, i = unpack_bool(buf, i)
		if have_title:
			self.title, i = unpack_bytes(buf, i)

		have_data, i = unpack_bool(buf, i)
		if have_data:
			self.data, i = unpack_bytes(buf, i)

		have_stamp, i = unpack_bool(buf, i)
		if have_stamp:
			self.stamp, i = unpack_u64(buf, i)

		have_prio, i = unpack_bool(buf, i)
		if have_prio:
			self.prio, i = unpack_u64(buf, i)

		have_redirect, i = unpack_bool(buf, i)
		if have_redirect:
			self.redirect, i = unpack_bytes(buf, i)

		have_nav_stop, i = unpack_bool(buf, i)
		if have_nav_stop:
			self.nav_stop, i = unpack_bool(buf, i)

		have_nav_label, i = unpack_bool(buf, i)
		if have_nav_label:
			self.nav_label, i = unpack_bytes(buf, i)

		return self

class MsgHeaders:
	def __init__(self, data):
		self.data = data

	@classmethod
	def unpack(cls, buf, i):
		data, i = unpack_bytes(buf, i)
		self = cls(data=data)
		return self

class MsgSubPages:
	def __init__(self, pages, nav_labels, prios):
		self.pages = pages
		self.nav_labels = nav_labels
		self.prios = prios

	@classmethod
	def unpack(cls, buf, i):
		self = cls(pages=[], nav_labels=[], prios=[])
		count, i = unpack_u64(buf, i)
		for _ in range(count):
			page, i = unpack_bytes(buf, i)
			self.pages.append(page)
		count, i = unpack_u64(buf, i)
		for _ in range(count):
			nav_label, i = unpack_bytes(buf, i)
			self.nav_labels.append(nav_label)
		count, i = unpack_u64(buf, i)
		for _ in range(count):
			prio, i = unpack_u64(buf, i)
			self.prios.append(prio)
		if (len(self.pages) != len(self.nav_labels) or
		    len(self.nav_labels) != len(self.prios)):
			raise ValueError("MsgSubPages: Invalid list size.")
		return self

class MsgMacro:
	def __init__(self, data):
		self.data = data

	@classmethod
	def unpack(cls, buf, i):
		data, i = unpack_bytes(buf, i)
		self = cls(data=data)
		return self

class MsgString:
	def __init__(self, data):
		self.data = data

	@classmethod
	def unpack(cls, buf, i):
		data, i = unpack_bytes(buf, i)
		self = cls(data=data)
		return self

class MsgRunPostHandler:
	pass#TODO

class MsgPostHandlerResult:
	pass#TODO

def unpack_message(buf, magic):
	variant, i = unpack_u32(buf, MSG_HDR_LEN)
	if magic == MAGIC_DB:
		if variant == ID_DB_PAGE:
			return MsgPage.unpack(buf, i)
		elif variant == ID_DB_HEADERS:
			return MsgHeaders.unpack(buf, i)
		elif variant == ID_DB_SUBPAGES:
			return MsgSubPages.unpack(buf, i)
		elif variant == ID_DB_MACRO:
			return MsgMacro.unpack(buf, i)
		elif variant == ID_DB_STRING:
			return MsgString.unpack(buf, i)
	elif magic == MAGIC_POST:
		if variant == ID_POST_RUNPOSTHANDLER:
			pass#TODO
		elif variant == ID_POST_POSTHANDLERRESULT:
			pass#TODO
	raise ValueError("unpack_message: Unknown variant ID.")

def recv_message(sock, magic):
	buf = bytearray()
	recvLen = MSG_HDR_LEN
	while True:
		data = sock.recv(recvLen)
		if not data:
			raise ValueError("recv_message: Peer disconnected.")
		buf += data
		if len(buf) >= MSG_HDR_LEN:
			try:
				plLen = unpack_header(buf, magic)
				if len(buf) - MSG_HDR_LEN >= plLen:
					return unpack_message(buf, magic)
				recvLen = MSG_HDR_LEN + plLen - len(buf)
			except IndexError:
				raise ValueError("recv_message: Buffer out of bounds access.")
		else:
			recvLen = MSG_HDR_LEN - len(buf)
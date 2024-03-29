#!/usr/bin/env python3

import os, re
from setuptools import setup

import setup_cython


def getEnvInt(name, default=0):
	try:
		return int(os.getenv(name, "%d" % default))
	except ValueError:
		return default

def getEnvBool(name, default=False):
	return bool(getEnvInt(name, 1 if default else 0))

def pyCythonPatchLine(line):
	# Patch the import statements
	line = re.sub(r'^(\s*from cms)(\.[0-9a-zA-Z_\.]+)? (c?import)', r'\1_cython\2 \3', line)
	line = re.sub(r'^(\s*c?import cms)(\.)?', r'\1_cython\2', line)
	return line

buildCython = getEnvBool("CMS_CYTHON_BUILD", True)
setup_cython.parallelBuild = getEnvBool("CMS_CYTHON_PARALLEL", True)
setup_cython.profileEnabled = getEnvBool("CMS_PROFILE")
setup_cython.debugEnabled = getEnvBool("CMS_DEBUG_BUILD")
setup_cython.pyCythonPatchLine = pyCythonPatchLine

cmdclass = {}

if buildCython:
	buildCython = setup_cython.cythonBuildPossible()
if buildCython:
	cmdclass["build_ext"] = setup_cython.CythonBuildExtension
	setup_cython.registerCythonModules()
else:
	print("Skipping build of CYTHON modules.")

ext_modules = setup_cython.ext_modules

setup(	name		= "cms",
	version		= "0.0",
	description	= "simple WSGI/Python based CMS script",
	license		= "GNU General Public License v2 or later",
	author		= "Michael Buesch",
	author_email	= "m@bues.ch",
	url		= "https://bues.ch",
	packages	= [ "cms", ],
	scripts		= [ "index.wsgi", "cms-cli", ],
	cmdclass	= cmdclass,
	ext_modules	= ext_modules,
	keywords	= "CMS WSGI Apache httpd",
	classifiers	= [
		"Development Status :: 5 - Production/Stable",
		"Environment :: Console",
		"Intended Audience :: Developers",
		"Intended Audience :: Other Audience",
		"License :: OSI Approved :: GNU General Public License v2 or later (GPLv2+)",
		"Operating System :: POSIX",
		"Operating System :: POSIX :: Linux",
		"Programming Language :: Cython",
		"Programming Language :: Python",
		"Programming Language :: Python :: 3",
		"Programming Language :: Python :: Implementation :: CPython",
		"Topic :: Database",
		"Topic :: Database :: Database Engines/Servers",
		"Topic :: Database :: Front-Ends",
		"Topic :: Internet",
		"Topic :: Internet :: WWW/HTTP",
		"Topic :: Internet :: WWW/HTTP :: Browsers",
		"Topic :: Internet :: WWW/HTTP :: Dynamic Content",
		"Topic :: Internet :: WWW/HTTP :: Site Management",
		"Topic :: Internet :: WWW/HTTP :: WSGI",
		"Topic :: Text Processing",
		"Topic :: Text Processing :: Markup :: HTML",
	],
	long_description = "simple WSGI/Python based CMS script"
)

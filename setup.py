#!/usr/bin/env python3

import re
from distutils.core import setup

import setup_cython


def pyCythonPatchLine(line):
	# Patch the import statements
	line = re.sub(r'^(\s*from cms[0-9a-zA-Z_]*)\.([0-9a-zA-Z_\.]+) import', r'\1_cython.\2 import', line)
	line = re.sub(r'^(\s*from cms[0-9a-zA-Z_]*)\.([0-9a-zA-Z_\.]+) cimport', r'\1_cython.\2 cimport', line)
	line = re.sub(r'^(\s*import cms[0-9a-zA-Z_]*)\.', r'\1_cython.', line)
	line = re.sub(r'^(\s*cimport cms[0-9a-zA-Z_]*)\.', r'\1_cython.', line)
	line = line.replace("Python based", "Cython based")
	return line

setup_cython.parallelBuild = True
setup_cython.pyCythonPatchLine = pyCythonPatchLine

cmdclass = {}

if setup_cython.cythonBuildPossible():
	cmdclass["build_ext"] = setup_cython.CythonBuildExtension
	setup_cython.registerCythonModules()

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
	keywords	= [ "CMS", "WSGI", ],
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

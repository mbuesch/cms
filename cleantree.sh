#!/bin/sh

basedir="$(dirname "$0")"
[ "$(echo "$basedir" | cut -c1)" = '/' ] || basedir="$PWD/$basedir"

set -e

if ! [ -x "$basedir/setup.py" ]; then
	echo "basedir sanity check failed"
	exit 1
fi

cd "$basedir"

find . \( \
	\( -name '__pycache__' \) -o \
	\( -name '*.pyo' \) -o \
	\( -name '*.pyc' \) -o \
	\( -name '*$py.class' \) \
       \) -delete

rm -f cms_cython
rm -rf build dist .pybuild
rm -f MANIFEST

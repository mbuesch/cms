#!/bin/sh

basedir="$(dirname "$0")"
[ "$(echo "$basedir" | cut -c1)" = '/' ] || basedir="$PWD/$basedir"

set -e

if ! [ -x "$basedir/setup.py" ]; then
	echo "basedir sanity check failed"
	exit 1
fi

cd "$basedir"

rm -f cms_cython
export CFLAGS="$CFLAGS -O3"
python3 ./setup.py build
ln -s build/lib.*-3*/cms_cython .

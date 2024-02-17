#!/bin/sh
# -*- coding: utf-8 -*-

basedir="$(realpath "$0" | xargs dirname)"

set -e

if ! [ -x "$basedir/setup.py" -a -f "$basedir/Cargo.toml" ]; then
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
rm -rf target
rm -f Cargo.lock cms-fsd.sock

# vim: ts=4 sw=4 expandtab

#!/bin/sh
# -*- coding: utf-8 -*-

basedir="$(realpath "$0" | xargs dirname)"

info()
{
    echo "--- $*"
}

error()
{
    echo "=== ERROR: $*" >&2
}

warning()
{
    echo "=== WARNING: $*" >&2
}

die()
{
    error "$*"
    exit 1
}

[ -x "$basedir/setup.py" -a -f "$basedir/Cargo.toml" ] ||\
    die "basedir sanity check failed"

cd "$basedir" || die "cd basedir failed."

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

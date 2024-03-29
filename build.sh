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

[ "$(id -u)" = "0" ] &&\
    die "Must NOT be root to build CMS."

cd "$basedir" || die "cd basedir failed."
rm -f cms_cython
export CFLAGS="-O3 -pipe"
export CPPFLAGS=
export CXXFLAGS=
python3 ./setup.py build || die "Python build failed."
ln -s build/lib.*-3*/cms_cython . || die "Python link failed."
cargo update || die "Cargo update failed."
cargo build --release || die "Cargo build failed."
cargo audit --deny warnings || die "Cargo audit failed."

# vim: ts=4 sw=4 expandtab

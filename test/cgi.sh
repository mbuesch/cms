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

release="debug"
get_path="/"
while [ $# -ge 1 ]; do
    case "$1" in
        --debug|-d)
            release="debug"
            ;;
        --release|-r)
            release="release"
            ;;
        *)
            get_path="$1"
            ;;
    esac
    shift
done

target="$basedir/../target/$release"
binary="$target/cms-cgi"
[ -x "$binary" ] || die "cms-cgi binary $binary not found."

rundir="$basedir/run"
export QUERY_STRING=
export REQUEST_METHOD=GET
export PATH_INFO="$get_path"
export CONTENT_LENGTH=
export CONTENT_TYPE=
export HTTPS=on
export HTTP_HOST=example.com
export HTTP_COOKIE=

"$binary" --rundir "$rundir" || die "cms-cgi failed."

# vim: ts=4 sw=4 expandtab

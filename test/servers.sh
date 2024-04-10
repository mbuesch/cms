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

cleanup()
{
    [ -n "$pid_backd" ] && kill "$pid_backd" 2>/dev/null
    [ -n "$pid_postd" ] && kill "$pid_postd" 2>/dev/null
    [ -n "$pid_fsd" ] && kill "$pid_fsd" 2>/dev/null
    wait
}

cleanup_and_exit()
{
    cleanup
    exit 1
}

trap cleanup_and_exit INT TERM
trap cleanup EXIT

start_fsd()
{
    echo "Starting cms-fsd..."
    rm -f "$rundir/cms-fsd.sock"
    local binary="$basedir/../target/release/cms-fsd"
    [ -x "$binary" ] || die "cms-fsd binary $binary not found."
    "$binary" \
        --rundir "$rundir" \
        "$dbdir" \
        &
    pid_fsd=$!
}

start_postd()
{
    echo "Starting cms-postd..."
    rm -f "$rundir/cms-postd.sock"
    local binary="$basedir/../target/release/cms-postd"
    [ -x "$binary" ] || die "cms-postd binary $binary not found."
    "$binary" \
        --rundir "$rundir" \
        "$dbdir" \
        &
    pid_postd=$!
}

start_backd()
{
    echo "Starting cms-backd..."
    export NOTIFY_SOCKET=
    rm -f "$rundir/cms-backd.sock"
    local pyscript="$basedir/../cmsbackpy/cms-backd"
    [ -r "$pyscript" ] || die "cms-backd script $pyscript not found."
    python3 "$pyscript" \
        --rundir "$rundir" \
        --pythonpath "$basedir/.." \
        --no-cython \
        &
    pid_backd=$!
}

rundir="$basedir/run"
dbdir="$basedir/../example/db"
pid_fsd=
pid_postd=
pid_backd=
mkdir -p "$rundir"
start_fsd
start_postd
sleep 1
start_backd
wait

# vim: ts=4 sw=4 expandtab

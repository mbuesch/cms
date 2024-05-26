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
    local binary="$target/cms-fsd"
    [ -x "$binary" ] || die "cms-fsd binary $binary not found."
    "$binary" \
        --rundir "$rundir" \
        --no-systemd \
        "$dbdir" \
        &
    pid_fsd=$!
}

start_postd()
{
    echo "Starting cms-postd..."
    rm -f "$rundir/cms-postd.sock"
    local binary="$target/cms-postd"
    [ -x "$binary" ] || die "cms-postd binary $binary not found."
    "$binary" \
        --rundir "$rundir" \
        --no-systemd \
        "$dbdir" \
        &
    pid_postd=$!
}

start_backd()
{
    echo "Starting cms-backd..."
    rm -f "$rundir/cms-backd.sock"
    local binary="$target/cms-backd"
    [ -x "$binary" ] || die "cms-backd binary $binary not found."
    "$binary" \
        --rundir "$rundir" \
        --no-systemd \
        &
    pid_backd=$!
}

release="debug"
while [ $# -ge 1 ]; do
    case "$1" in
        --debug|-d)
            release="debug"
            ;;
        --release|-r)
            release="release"
            ;;
        *)
            die "Invalid option: $1"
            ;;
    esac
    shift
done

target="$basedir/../target/$release"
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

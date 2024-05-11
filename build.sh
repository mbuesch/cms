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
export CFLAGS="-O3 -pipe"
export CPPFLAGS=
export CXXFLAGS=
python3 ./setup.py build || die "Python build failed."
cargo build || die "Cargo build (debug) failed."
cargo test || die "Cargo test failed."
cargo auditable build --release || die "Cargo build (release) failed."
cargo audit bin --deny warnings \
    target/release/cms-backd \
    target/release/cms-cgi \
    target/release/cms-fsd \
    target/release/cms-postd \
    || die "Cargo audit failed."

# vim: ts=4 sw=4 expandtab

#!/bin/sh
# -*- coding: utf-8 -*-

basedir="$(realpath "$0" | xargs dirname)"

set -e

if ! [ -x "$basedir/setup.py" -a -f "$basedir/Cargo.toml" ]; then
    echo "basedir sanity check failed"
    exit 1
fi

cd "$basedir"

rm -f cms_cython
export CFLAGS="$CFLAGS -O3"
python3 ./setup.py build
ln -s build/lib.*-3*/cms_cython .
cargo update
cargo audit --deny warnings
cargo build --release

# vim: ts=4 sw=4 expandtab

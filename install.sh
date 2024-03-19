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

do_install()
{
    info "install $*"
    install "$@" || die "Failed install $*"
}

do_systemctl()
{
    info "systemctl $*"
    systemctl "$@" || die "Failed to systemctl $*"
}

try_systemctl()
{
    info "systemctl $*"
    systemctl "$@" 2>/dev/null
}

stop_services()
{
    try_systemctl stop apache2
    try_systemctl stop cms-fsd.socket
    try_systemctl stop cms-fsd.service
}

start_services()
{
    do_systemctl start cms-fsd.socket
    do_systemctl start apache2
}

install_dirs()
{
    do_install \
        -o root -g root -m 0755 \
        -d /opt/cms/bin

    do_install \
        -o root -g root -m 0755 \
        -d /opt/cms/etc/cms

    do_install \
        -o root -g root -m 0755 \
        -d /opt/cms/share/cms-wsgi

    do_install \
        -o root -g root -m 0755 \
        -d /opt/cms/lib/python3/site-packages/cms

    do_install \
        -o root -g root -m 0755 \
        -d /opt/cms/lib/python3/site-packages/cms_cython
}

install_fsd()
{
    do_install \
        -o root -g root -m 0755 \
        "$basedir/target/release/cms-fsd" \
        /opt/cms/bin/

    do_install \
        -o root -g root -m 0644 \
        "$basedir/cms-fsd/cms-fsd.service" \
        /etc/systemd/system/

    do_install \
        -o root -g root -m 0644 \
        "$basedir/cms-fsd/cms-fsd.socket" \
        /etc/systemd/system/

    do_systemctl enable cms-fsd.service
    do_systemctl enable cms-fsd.socket
}

install_py()
{
    do_install \
        -o root -g root -m 0644 \
        "$basedir"/cms/*.py \
        /opt/cms/lib/python3/site-packages/cms/

    do_install \
        -o root -g root -m 0644 \
        "$basedir"/cms_cython/*.py "$basedir"/cms_cython/*.so \
        /opt/cms/lib/python3/site-packages/cms_cython/

    do_install \
        -o root -g root -m 0644 \
        "$basedir/index.wsgi" \
        /opt/cms/share/cms-wsgi/
}

stop_services
install_dirs
install_fsd
install_py
start_services

# vim: ts=4 sw=4 expandtab
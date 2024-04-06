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

entry_checks()
{
    [ -d "$basedir/target/release" ] ||\
        die "CMS is not built! Run ./build.sh"

    [ "$(id -u)" = "0" ] ||\
        die "Must be root to install CMS."
}

stop_services()
{
    try_systemctl stop apache2
    try_systemctl stop cms-backd.socket
    try_systemctl stop cms-backd.service
    try_systemctl stop cms-postd.socket
    try_systemctl stop cms-postd.service
    try_systemctl stop cms-fsd.socket
    try_systemctl stop cms-fsd.service
}

start_services()
{
    do_systemctl start cms-fsd.socket
    do_systemctl start cms-postd.socket
    do_systemctl start cms-backd.socket
    do_systemctl start apache2
}

install_dirs()
{
    rm -rf /opt/cms/bin
    rm -rf /opt/cms/lib
    rm -rf /opt/cms/libexec
    rm -rf /opt/cms/share

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
        -d /opt/cms/libexec/cms-cgi

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

install_postd()
{
    do_install \
        -o root -g root -m 0755 \
        "$basedir/target/release/cms-postd" \
        /opt/cms/bin/

    do_install \
        -o root -g root -m 0644 \
        "$basedir/cms-postd/cms-postd.service" \
        /etc/systemd/system/

    do_install \
        -o root -g root -m 0644 \
        "$basedir/cms-postd/cms-postd.socket" \
        /etc/systemd/system/

    do_systemctl enable cms-postd.service
    do_systemctl enable cms-postd.socket
}

install_cgi()
{
    do_install \
        -o root -g root -m 0755 --no-target-directory \
        "$basedir/target/release/cms-cgi" \
        /opt/cms/libexec/cms-cgi/cms.cgi
}

install_py()
{
    do_install \
        -o root -g root -m 0644 \
        "$basedir"/cms/*.py \
        /opt/cms/lib/python3/site-packages/cms/

    do_install \
        -o root -g root -m 0755 \
        "$basedir/cmsbackpy/cms-backd" \
        /opt/cms/bin/

    do_install \
        -o root -g root -m 0644 \
        "$basedir/cmsbackpy/cms-backd.service" \
        /etc/systemd/system/

    do_install \
        -o root -g root -m 0644 \
        "$basedir/cmsbackpy/cms-backd.socket" \
        /etc/systemd/system/

    do_install \
        -o root -g root -m 0644 \
        "$basedir"/cms_cython/*.py "$basedir"/cms_cython/*.so \
        /opt/cms/lib/python3/site-packages/cms_cython/

    do_install \
        -o root -g root -m 0644 \
        "$basedir/index.wsgi" \
        /opt/cms/share/cms-wsgi/

    do_systemctl enable cms-backd.service
    do_systemctl enable cms-backd.socket
}

entry_checks
stop_services
install_dirs
install_fsd
install_postd
install_cgi
install_py
start_services

# vim: ts=4 sw=4 expandtab

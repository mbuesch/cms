#!/bin/sh
# -*- coding: utf-8 -*-

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

sys_groupadd()
{
    local args="--system"
    info "groupadd $args $*"
    groupadd $args "$@" || die "Failed groupadd"
}

sys_useradd()
{
    local args="--system -s /usr/sbin/nologin -d /nonexistent -M -N"
    info "useradd $args $*"
    useradd $args "$@" || die "Failed useradd"
}

do_usermod()
{
    info "usermod $*"
    usermod "$@" || die "Failed usermod"
}

# Stop the daemons.
systemctl stop cms-fsd.socket >/dev/null 2>&1
systemctl stop cms-fsd.service >/dev/null 2>&1
systemctl stop cms-postd.socket >/dev/null 2>&1
systemctl stop cms-postd.service >/dev/null 2>&1

# Delete all existing users, if any.
userdel cms-fsd >/dev/null 2>&1
userdel cms-postd >/dev/null 2>&1
userdel cms-backd >/dev/null 2>&1

# Delete all existing groups, if any.
groupdel cms-fsd >/dev/null 2>&1
groupdel cms-postd >/dev/null 2>&1
groupdel cms-backd >/dev/null 2>&1
groupdel cms-fs-ro >/dev/null 2>&1
groupdel cms-fs-x >/dev/null 2>&1
groupdel cms-sock-db >/dev/null 2>&1
groupdel cms-sock-post >/dev/null 2>&1
groupdel cms-sock-back >/dev/null 2>&1

# Create system groups.
sys_groupadd cms-fsd
sys_groupadd cms-postd
sys_groupadd cms-backd
sys_groupadd cms-fs-ro
sys_groupadd cms-fs-x
sys_groupadd cms-sock-db
sys_groupadd cms-sock-post
sys_groupadd cms-sock-back

# Create system users.
sys_useradd -G cms-sock-db,cms-fs-ro -g cms-fsd cms-fsd
sys_useradd -G cms-sock-post,cms-fs-x -g cms-postd cms-postd
sys_useradd -G cms-sock-back,cms-sock-db,cms-sock-post -g cms-backd cms-backd

# Add the communication socket to the web server process user.
do_usermod -a -G cms-sock-db www-data #TODO: cms-sock-db shall be removed eventually.
do_usermod -a -G cms-sock-post www-data #TODO: cms-sock-post shall be removed eventually.
do_usermod -a -G cms-sock-back www-data

# The git-user shall be able to give group permissions in db.
if grep -q '^git:' /etc/passwd; then
    do_usermod -a -G cms-fs-ro,cms-fs-x,www-data git
fi

# vim: ts=4 sw=4 expandtab

// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use anyhow::{self as ah, format_err as err, Context as _};
use std::os::{
    fd::{FromRawFd as _, RawFd},
    unix::net::UnixListener,
};
use systemd::daemon;

/// Check if there is a systemd unix socket fd that is usable.
fn check_fd(fd: RawFd) -> bool {
    let no_path: Option<std::ffi::CString> = None;
    daemon::is_socket_unix(
        fd,
        Some(daemon::SocketType::Stream),
        daemon::Listening::NoListeningCheck,
        no_path,
    )
    .unwrap_or(false)
}

/// Check whether we have been invoked by systemd.
pub fn have_systemd() -> bool {
    daemon::listen_fds(false)
        .map(|fds| fds.iter().any(check_fd))
        .unwrap_or(false)
}

/// Create a new [UnixListener] with the socket provided by systemd.
///
/// If [unset_environment] is true, all environment variables related
/// to this operation will be cleared.
pub fn unix_from_systemd(unset_environment: bool) -> ah::Result<UnixListener> {
    let fds = daemon::listen_fds(unset_environment).context("Systemd listen_fds")?;
    for fd in fds.iter() {
        if check_fd(fd) {
            // SAFETY:
            // The fd from systemd is good and lives for the lifetime of the program.
            return Ok(unsafe { UnixListener::from_raw_fd(fd) });
        }
    }
    Err(err!("No systemd unix socket fd found"))
}

/// Notify ready-status to systemd.
///
/// If [unset_environment] is true, all environment variables related
/// to this operation will be cleared.
pub fn systemd_notify_ready(unset_environment: bool) -> ah::Result<()> {
    daemon::notify(unset_environment, [(daemon::STATE_READY, "1")].iter())
        .context("Systemd notify READY=1")
        .map(|_| ())
}

// vim: ts=4 sw=4 expandtab

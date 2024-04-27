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

use anyhow::{self as ah, Context as _};
use std::os::{fd::FromRawFd as _, unix::net::UnixListener};

/// Create a new [UnixListener] with the socket provided by systemd.
///
/// All environment variables related to this operation will be cleared.
pub fn unix_from_systemd() -> ah::Result<Option<UnixListener>> {
    if sd_notify::booted().unwrap_or(false) {
        let mut fds = sd_notify::listen_fds().context("Systemd listen_fds")?;
        if let Some(fd) = fds.next() {
            // SAFETY:
            // The fd from systemd is good and lives for the lifetime of the program.
            return Ok(Some(unsafe { UnixListener::from_raw_fd(fd) }));
        }
    }
    Ok(None)
}

/// Notify ready-status to systemd.
///
/// All environment variables related to this operation will be cleared.
pub fn systemd_notify_ready() -> ah::Result<()> {
    sd_notify::notify(true, &[sd_notify::NotifyState::Ready])?;
    Ok(())
}

// vim: ts=4 sw=4 expandtab

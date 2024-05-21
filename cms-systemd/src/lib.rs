// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

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

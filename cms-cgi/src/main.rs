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

#![forbid(unsafe_code)]

mod cgi;

use crate::cgi::Cgi;
use anyhow::{self as ah, Context as _};
use std::time::Duration;
use tokio::runtime;

async fn async_main() -> ah::Result<()> {
    let cgi = Cgi::new();
    cgi.run()
}

fn main() -> ah::Result<()> {
    runtime::Builder::new_current_thread()
        .thread_keep_alive(Duration::from_millis(0))
        .enable_all()
        .build()
        .context("Tokio runtime builder")?
        .block_on(async_main())
}

// vim: ts=4 sw=4 expandtab

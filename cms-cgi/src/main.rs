// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2025 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

mod cgi;

use crate::cgi::Cgi;
use anyhow as ah;
use std::path::Path;

const RUNDIR: &str = "/run";

fn main() -> ah::Result<()> {
    let mut cgi = Cgi::new(Path::new(RUNDIR))?;
    cgi.run();
    Ok(())
}

// vim: ts=4 sw=4 expandtab

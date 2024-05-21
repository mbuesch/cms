// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

mod cgi;

use crate::cgi::Cgi;
use anyhow as ah;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
struct Opts {
    /// The run directory for runtime data.
    #[arg(long, default_value = "/run")]
    rundir: PathBuf,
}

fn main() -> ah::Result<()> {
    let opts = Opts::parse();
    let mut cgi = Cgi::new(&opts.rundir)?;
    cgi.run();
    Ok(())
}

// vim: ts=4 sw=4 expandtab

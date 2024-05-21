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

use build_target::target_arch;
use cms_seccomp::{seccomp_compile_for_arch, Action, Allow};
use std::{env, fs::OpenOptions, io::Write, path::Path};

fn main() {
    let arch = target_arch().expect("Failed to get build target architecture");

    let seccomp_filter = seccomp_compile_for_arch(
        &[
            Allow::Read,
            Allow::Write,
            Allow::Recv,
            Allow::Send,
            Allow::Mmap,
        ],
        Action::Kill,
        arch.as_str(),
    )
    .expect("Failed to compile seccomp filter")
    .serialize();

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not set");
    let mut filter_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(Path::new(&out_dir).join("seccomp_filter.bpf"))
        .expect("Failed to open seccomp_filter.bpf");
    filter_file
        .write_all(&seccomp_filter)
        .expect("Failed to write seccomp_filter.bpf");
}

// vim: ts=4 sw=4 expandtab

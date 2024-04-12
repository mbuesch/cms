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

use build_target::target_arch;
use cms_seccomp::{seccomp_compile_for_arch, Action, Allow};
use std::{env, fs::OpenOptions, io::Write, path::Path};

fn main() {
    let arch = target_arch().expect("Failed to get build target architecture");

    let seccomp_filter = seccomp_compile_for_arch(
        &[Allow::Read, Allow::Write, Allow::Recv, Allow::Send],
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

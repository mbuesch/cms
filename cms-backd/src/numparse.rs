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

use anyhow::{self as ah, format_err as err};

pub fn parse_usize(s: &str) -> ah::Result<usize> {
    let s = s.trim();
    if let Some(s) = s.strip_prefix("0x") {
        Ok(usize::from_str_radix(s, 16)?)
    } else {
        Ok(s.parse::<usize>()?)
    }
}

pub fn parse_i64(s: &str) -> ah::Result<i64> {
    let s = s.trim();
    if let Some(s) = s.strip_prefix("0x") {
        Ok(i64::from_str_radix(s, 16)?)
    } else {
        Ok(s.parse::<i64>()?)
    }
}

pub fn parse_f64(s: &str) -> ah::Result<f64> {
    Ok(s.trim().parse::<f64>()?)
}

pub fn parse_bool(s: &str) -> ah::Result<bool> {
    let s = s.to_lowercase();
    let s = s.trim();
    match s {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(err!("Invalid boolean string")),
    }
}

// vim: ts=4 sw=4 expandtab

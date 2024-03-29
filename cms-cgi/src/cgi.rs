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
use std::{env, ffi::OsString};

fn get_cgienv(name: &str) -> OsString {
    env::var_os(name).unwrap_or_else(|| OsString::new())
}

fn get_cgienv_str(name: &str) -> String {
    get_cgienv(name)
        .into_string()
        .unwrap_or_else(|_| String::new())
}

fn get_cgienv_u32(name: &str, default: u32) -> u32 {
    u32::from_str_radix(&get_cgienv_str(name), 10).unwrap_or(default)
}

pub struct Cgi {
    query: OsString,
    meth: OsString,
    path: OsString,
    body_len: u32,
    body_type: OsString,
    https: bool,
    host: OsString,
    cookie: OsString,
}

impl Cgi {
    pub fn new() -> Self {
        let query = get_cgienv("QUERY_STRING");
        let meth = get_cgienv("REQUEST_METHOD");
        let path = get_cgienv("PATH_INFO");
        let body_len = get_cgienv_u32("CONTENT_LENGTH", 0);
        let body_type = get_cgienv("CONTENT_TYPE");
        let https = get_cgienv("HTTPS").as_encoded_bytes() == b"on";
        let host = get_cgienv("HTTP_HOST");
        let cookie = get_cgienv("HTTP_COOKIE");
        Self {
            query,
            meth,
            path,
            body_len,
            body_type,
            https,
            host,
            cookie,
        }
    }

    pub fn run(&self) -> ah::Result<()> {
        match self.meth.as_encoded_bytes() {
            b"GET" => self.run_get(),
            b"POST" => self.run_post(),
            _ => Err(err!(
                "Unsupported REQUEST_METHOD: '{}'",
                self.meth.to_string_lossy()
            )),
        }
    }

    fn run_get(&self) -> ah::Result<()> {
        //TODO
        Ok(())
    }

    fn run_post(&self) -> ah::Result<()> {
        if self.body_len == 0 {
            return Err(err!("POST: Invalid CONTENT_LENGTH."));
        }
        if self.body_type.is_empty() {
            return Err(err!("POST: Invalid CONTENT_TYPE."));
        }
        //TODO
        Ok(())
    }
}

// vim: ts=4 sw=4 expandtab

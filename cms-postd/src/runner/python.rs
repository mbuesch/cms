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

use crate::{reply::Reply, request::Request};
use anyhow as ah;
use pyo3::prelude::*;

pub struct PyRunner {
    _priv: (),
}

impl PyRunner {
    pub fn new() -> Self {
        Self { _priv: () }
    }

    pub fn run(&mut self, request: &Request) -> ah::Result<Reply> {
        Ok(Python::with_gil(|py| -> PyResult<Reply> {
            //TODO

            Ok(Reply {
                body: b"".to_vec(),
                mime: "".to_string(),
            })
        })?)
    }
}

// vim: ts=4 sw=4 expandtab

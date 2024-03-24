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

use crate::{reply::Reply, request::Request, runner::Runner};
use anyhow::{self as ah, Context as _};
use cms_ident::{Strip, Tail};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyString},
};
use std::path::{Path, PathBuf};

fn sanitize_python_module_name_char(c: char) -> char {
    const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
    if UPPERCASE.contains(c) || LOWERCASE.contains(c) {
        c
    } else {
        '_'
    }
}

pub struct PyRunner {
    db_path: PathBuf,
}

impl PyRunner {
    pub fn new(db_path: &Path) -> Self {
        Self {
            //TODO: We should store the post handlers in a separate hierarchy.
            db_path: db_path.join("pages"),
        }
    }
}

impl Runner for PyRunner {
    fn run(&mut self, request: &Request) -> ah::Result<Reply> {
        Ok(Python::with_gil(|py| -> PyResult<Reply> {
            let path = &request.path;

            // We only support execution of post.py.
            if path.last_element_str().unwrap_or("") != "post.py" {
                return Err(ah::format_err!("PyRunner: Handler file not supported.").into());
            }
            //TODO check if the file exists, here in Rust code.

            // Path to the directory containing the post.py.
            let mod_dir = path
                .to_stripped_fs_path(&self.db_path, Strip::Right(1), &Tail::None)
                .context("Get module directory")?;
            let mod_dir_str = mod_dir
                .as_os_str()
                .to_str()
                .context("Post-module directory to str conversion")?;

            // Get the sanitized and checked fs path to the module.
            let mod_path = path.to_fs_path(&self.db_path, &Tail::None);
            let mod_path_str = mod_path
                .as_os_str()
                .to_str()
                .context("Post-module path to str conversion")?;
            // Create a module name from its path.
            let mod_name: String = mod_path_str
                .chars()
                .map(sanitize_python_module_name_char)
                .collect();

            // Create Python objects for locals context.
            let request_query = PyDict::new(py);
            for (k, v) in request.query.iter() {
                request_query
                    .set_item(PyString::new(py, k), PyBytes::new(py, v))
                    .context("Request query to Python")?;
            }
            let request_form_fields = PyDict::new(py);
            for (k, v) in request.form_fields.iter() {
                request_form_fields
                    .set_item(PyString::new(py, k), PyBytes::new(py, v))
                    .context("Request form-fields to Python")?;
            }
            let handler_mod_path = PyString::new(py, mod_path_str);
            let handler_mod_name = PyString::new(py, &mod_name);
            let handler_mod_dir = PyString::new(py, mod_dir_str);

            // Prepare Python locals context dict.
            let locals = PyDict::new(py);
            locals
                .set_item("handler_mod_name", handler_mod_name)
                .context("Construct Python locals")?;
            locals
                .set_item("handler_mod_path", handler_mod_path)
                .context("Construct Python locals")?;
            locals
                .set_item("handler_mod_dir", handler_mod_dir)
                .context("Construct Python locals")?;
            locals
                .set_item("request_query", request_query)
                .context("Construct Python locals")?;
            locals
                .set_item("request_form_fields", request_form_fields)
                .context("Construct Python locals")?;
            locals
                .set_item("reply_body", PyBytes::new(py, b""))
                .context("Construct Python locals")?;
            locals
                .set_item("reply_mime", PyString::new(py, ""))
                .context("Construct Python locals")?;

            //TODO pyo3 can't do subinterpreters. As workaround run the handler with multiprocessing and poll the result with the gil released.

            // Run the Python post handler.
            py.run(include_str!("python_stub.py"), None, Some(locals))
                .context("Python run")?;

            // Extract the reply body from locals.
            let Some(reply_body) = locals.get_item("reply_body").context("reply_body")? else {
                return Err(ah::format_err!("PyRunner: reply_body not in Python locals.").into());
            };
            let Ok(reply_body): Result<&PyBytes, _> = reply_body.downcast() else {
                return Err(ah::format_err!("PyRunner: reply_body not Python 'bytes'.").into());
            };
            let reply_body = reply_body.as_bytes().to_vec();
            if reply_body.is_empty() {
                return Err(ah::format_err!("PyRunner: reply_body is empty.").into());
            }

            // Extract the reply mime from locals.
            let Some(reply_mime) = locals.get_item("reply_mime").context("reply_mime")? else {
                return Err(ah::format_err!("PyRunner: reply_mime not in Python locals.").into());
            };
            let Ok(reply_mime): Result<&PyString, _> = reply_mime.downcast() else {
                return Err(ah::format_err!("PyRunner: reply_mime not Python 'str'.").into());
            };
            let reply_mime = reply_mime
                .to_str()
                .context("PyRunner: Invalid reply_mime 'str' encoding")?
                .to_string();
            if reply_mime.is_empty() {
                return Err(ah::format_err!("PyRunner: reply_mime is empty.").into());
            }

            Ok(Reply {
                body: reply_body,
                mime: reply_mime,
            })
        })?)
    }
}

// vim: ts=4 sw=4 expandtab

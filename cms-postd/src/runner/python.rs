// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{reply::Reply, request::Request, runner::Runner};
use anyhow::{self as ah, format_err as err, Context as _};
use cms_ident::{Strip, Tail};
use pyo3::{
    create_exception,
    exceptions::PyException,
    prelude::*,
    types::{PyBytes, PyDict, PyString},
};
use std::{ffi::CString, os::unix::fs::PermissionsExt as _, path::Path};
use tokio::{fs, task};

fn sanitize_python_module_name_char(c: char) -> char {
    const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
    if UPPERCASE.contains(c) || LOWERCASE.contains(c) {
        c
    } else {
        '_'
    }
}

create_exception!(
    cms_exceptions,
    CMSPostException,
    PyException,
    "CMS POST handler error"
);

pub struct PyRunner<'a> {
    db_post_path: &'a Path,
}

impl<'a> PyRunner<'a> {
    pub fn new(db_post_path: &'a Path) -> Self {
        Self { db_post_path }
    }
}

impl<'a> Runner for PyRunner<'a> {
    async fn run(&mut self, request: Request) -> ah::Result<Reply> {
        // We only support execution of post.py.
        if request.path.last_element_str().unwrap_or("") != "post.py" {
            return Err(err!("PyRunner: Handler file not supported."));
        }

        // Path to the directory containing the post.py.
        let mod_dir = request
            .path
            .to_stripped_fs_path(self.db_post_path, Strip::Right(1), &Tail::None)
            .context("Get module directory")?;
        let mod_dir_string = mod_dir
            .as_os_str()
            .to_str()
            .context("Post-module directory to str conversion")?
            .to_string();

        // Get the sanitized and checked fs path to the module.
        let mod_path = request.path.to_fs_path(self.db_post_path, &Tail::None);
        let mod_path_string = mod_path
            .as_os_str()
            .to_str()
            .context("Post-module path to str conversion")?
            .to_string();
        // Create a module name from its path.
        let mod_name: String = mod_path_string
            .chars()
            .map(sanitize_python_module_name_char)
            .collect();

        // Check post.py file mode:
        // group: rx, not w
        // other: not w
        {
            let mod_fd = fs::File::open(&mod_path)
                .await
                .context("post.py not readable")?;
            let meta = mod_fd.metadata().await.context("post.py metadata read")?;
            let mode = meta.permissions().mode();
            if mode & 0o070 != 0o050 {
                return Err(err!(
                    "PyRunner: post.py is not group-read-execute file mode"
                ));
            }
            if mode & 0o002 != 0o000 {
                return Err(err!(
                    "PyRunner: post.py must not have other-write file mode."
                ));
            }
        }

        // Spawn a blocking task for Python.
        let runner_task = task::spawn_blocking(move || {
            Ok(Python::with_gil(|py| -> PyResult<Reply> {
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
                let handler_mod_path = PyString::new(py, &mod_path_string);
                let handler_mod_name = PyString::new(py, &mod_name);
                let handler_mod_dir = PyString::new(py, &mod_dir_string);

                // Prepare Python locals context dict.
                let locals = PyDict::new(py);
                locals
                    .set_item("CMSPostException", py.get_type::<CMSPostException>())
                    .context("Construct Python locals")?;
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
                let runner_result = py.run(
                    &CString::new(include_str!("python_stub.py"))
                        .expect("python_stub.py CString decode failed"),
                    None,
                    Some(&locals),
                );

                // Handle post handler exception.
                match runner_result {
                    Ok(_) => (),
                    Err(e) if e.is_instance_of::<CMSPostException>(py) => {
                        // This is a CMSPostException.
                        // Send the message to the postd client.
                        return Ok(Reply {
                            error: format!("POST handler failed: {e}"),
                            body: b"".to_vec(),
                            mime: "".to_string(),
                        });
                    }
                    Err(e) => {
                        return Err(e).context("PyRunner: Execution failed")?;
                    }
                }

                // Extract the reply body from locals.
                let Some(reply_body) = locals.get_item("reply_body").context("reply_body")? else {
                    return Err(err!("PyRunner: reply_body not in Python locals.").into());
                };
                let Ok(reply_body): Result<&Bound<PyBytes>, _> = reply_body.downcast() else {
                    return Err(err!("PyRunner: reply_body not Python 'bytes'.").into());
                };
                let reply_body = reply_body.as_bytes().to_vec();
                if reply_body.is_empty() {
                    return Err(err!("PyRunner: reply_body is empty.").into());
                }

                // Extract the reply mime from locals.
                let Some(reply_mime) = locals.get_item("reply_mime").context("reply_mime")? else {
                    return Err(err!("PyRunner: reply_mime not in Python locals.").into());
                };
                let Ok(reply_mime): Result<&Bound<PyString>, _> = reply_mime.downcast() else {
                    return Err(err!("PyRunner: reply_mime not Python 'str'.").into());
                };
                let reply_mime = reply_mime
                    .to_str()
                    .context("PyRunner: Invalid reply_mime 'str' encoding")?
                    .to_string();
                if reply_mime.is_empty() {
                    return Err(err!("PyRunner: reply_mime is empty.").into());
                }

                Ok(Reply {
                    error: "".to_string(),
                    body: reply_body,
                    mime: reply_mime,
                })
            })?)
        });

        runner_task.await?
    }
}

// vim: ts=4 sw=4 expandtab

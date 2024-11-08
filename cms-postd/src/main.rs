// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![deny(unsafe_code)] // `deny` instead of `forbid`, because pyo3 uses `#[allow(unsafe_code)]` in macros.

mod reply;
mod request;
mod runner;

use crate::{
    request::Request,
    runner::{python::PyRunner, Runner},
};
use anyhow::{self as ah, format_err as err, Context as _};
use clap::Parser;
use cms_socket::{CmsSocket, CmsSocketConn, MsgSerde};
use cms_socket_post::{Msg, SOCK_FILE};
use std::{num::NonZeroUsize, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    runtime,
    signal::unix::{signal, SignalKind},
    sync, task,
};

#[derive(Parser, Debug, Clone)]
struct Opts {
    /// Path to the database directory.
    db_path: PathBuf,

    /// The run directory for runtime data.
    #[arg(long, default_value = "/run")]
    rundir: PathBuf,

    /// Always run in non-systemd mode.
    #[arg(long, default_value = "false")]
    no_systemd: bool,

    /// Set the number async worker threads.
    #[arg(long, default_value = "1")]
    worker_threads: NonZeroUsize,
}

async fn process_conn(mut conn: CmsSocketConn, opts: Arc<Opts>) -> ah::Result<()> {
    let db_post_path = opts.db_path.join("pages-post");

    loop {
        let msg = conn.recv_msg(Msg::try_msg_deserialize).await?;
        match msg {
            Some(Msg::RunPostHandler {
                path,
                query,
                form_fields,
            }) => {
                let request = Request {
                    //TODO: Cleaning should be done in the backd.
                    path: path.into_cleaned_path().into_checked()?,
                    query,
                    form_fields,
                };

                let reply_data = if request.path.ends_with(".py") {
                    let mut runner = PyRunner::new(&db_post_path);
                    runner.run(request).await?
                } else {
                    return Err(err!("RunPostHandler: Unknown handler type."));
                };

                let reply = Msg::PostHandlerResult {
                    error: reply_data.error,
                    body: reply_data.body,
                    mime: reply_data.mime,
                };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::PostHandlerResult { .. }) => {
                eprintln!("Received unsupported message.");
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("Client disconnected.");
                return Ok(());
            }
        }
    }
}

async fn async_main(opts: Arc<Opts>) -> ah::Result<()> {
    let (main_exit_tx, mut main_exit_rx) = sync::mpsc::channel(1);

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sighup = signal(SignalKind::hangup()).unwrap();

    let mut sock = CmsSocket::from_systemd_or_path(opts.no_systemd, &opts.rundir.join(SOCK_FILE))?;

    //TODO: install seccomp filter.

    // Task: Socket handler.
    let opts_clone = Arc::clone(&opts);
    task::spawn(async move {
        loop {
            let opts_clone = Arc::clone(&opts_clone);
            match sock.accept().await {
                Ok(conn) => {
                    // Socket connection handler.
                    task::spawn(async move {
                        if let Err(e) = process_conn(conn, opts_clone).await {
                            eprintln!("Client error: {e}");
                        }
                    });
                }
                Err(e) => {
                    let _ = main_exit_tx.send(Err(e)).await;
                    break;
                }
            }
        }
    });

    // Main task.
    let exitcode;
    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                eprintln!("SIGTERM: Terminating.");
                exitcode = Ok(());
                break;
            }
            _ = sigint.recv() => {
                exitcode = Err(err!("Interrupted by SIGINT."));
                break;
            }
            _ = sighup.recv() => {
                eprintln!("SIGHUP: Reloading.");
                // nothing to do.
            }
            code = main_exit_rx.recv() => {
                if let Some(code) = code {
                    exitcode = code;
                } else {
                    exitcode = Err(err!("Unknown error code."));
                }
                break;
            }
        }
    }
    exitcode
}

fn main() -> ah::Result<()> {
    let opts = Arc::new(Opts::parse());

    runtime::Builder::new_multi_thread()
        .worker_threads(opts.worker_threads.into())
        .max_blocking_threads(opts.worker_threads.into())
        .thread_keep_alive(Duration::from_millis(1000))
        .enable_all()
        .build()
        .context("Tokio runtime builder")?
        .block_on(async_main(opts))
}

// vim: ts=4 sw=4 expandtab

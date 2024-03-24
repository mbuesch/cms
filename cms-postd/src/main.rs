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

mod reply;
mod request;
mod runner;

use crate::{
    request::Request,
    runner::{python::PyRunner, Runner},
};
use anyhow::{self as ah, Context as _};
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
    loop {
        let opts = Arc::clone(&opts);
        let msg = conn.recv_msg(Msg::try_msg_deserialize).await?;
        match msg {
            Some(Msg::RunPostHandler {
                path,
                query,
                form_fields,
            }) => {
                let request = Request {
                    path: path.into_cleaned_path().into_checked()?,
                    query,
                    form_fields,
                };

                let post_task = task::spawn_blocking(move || {
                    if request.path.ends_with(".py") {
                        let mut runner = PyRunner::new(&opts.db_path);
                        Ok(runner.run(&request)?)
                    } else {
                        Err(ah::format_err!("RunPostHandler: Unknown handler type."))
                    }
                });
                let reply_data = post_task.await??;

                let reply = Msg::PostHandlerResult {
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
                exitcode = Err(ah::format_err!("Interrupted by SIGINT."));
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
                    exitcode = Err(ah::format_err!("Unknown error code."));
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
        .thread_keep_alive(Duration::from_millis(0))
        .worker_threads(opts.worker_threads.into())
        .enable_all()
        .build()
        .context("Tokio runtime builder")?
        .block_on(async_main(opts))
}

// vim: ts=4 sw=4 expandtab

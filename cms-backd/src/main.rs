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

use anyhow::{self as ah, format_err as err, Context as _};
use clap::Parser;
use cms_socket::CmsSocket;
use cms_socket_back::SOCK_FILE;
use std::{num::NonZeroUsize, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    runtime,
    signal::unix::{signal, SignalKind},
    sync, task,
};

#[derive(Parser, Debug, Clone)]
struct Opts {
    /// The run directory for runtime data.
    #[arg(long, default_value = "/run")]
    rundir: PathBuf,

    /// Always run in non-systemd mode.
    #[arg(long, default_value = "false")]
    no_systemd: bool,

    /// Set the number async worker threads.
    #[arg(long, default_value = "3")]
    worker_threads: NonZeroUsize,
}

async fn async_main(opts: Arc<Opts>) -> ah::Result<()> {
    let (main_exit_tx, mut main_exit_rx) = sync::mpsc::channel(1);

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sighup = signal(SignalKind::hangup()).unwrap();

    let mut sock = CmsSocket::from_systemd_or_path(opts.no_systemd, &opts.rundir.join(SOCK_FILE))?;

    //TODO install seccomp filter.

    // Task: Socket handler.
    task::spawn(async move {
        loop {
            match sock.accept().await {
                Ok(_conn) => {
                    // Socket connection handler.
                    //TODO
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
                //TODO
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
        .thread_keep_alive(Duration::from_millis(1000))
        .worker_threads(opts.worker_threads.into())
        .enable_all()
        .build()
        .context("Tokio runtime builder")?
        .block_on(async_main(opts))
}

// vim: ts=4 sw=4 expandtab

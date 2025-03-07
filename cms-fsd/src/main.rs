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

mod db_cache;
mod db_fsintf;

use crate::{db_cache::DbCache, db_fsintf::DbFsIntf};
use anyhow::{self as ah, Context as _, format_err as err};
use clap::Parser;
use cms_seccomp::{Action, Allow, Filter};
use cms_socket::{CmsSocket, CmsSocketConn, MsgSerde};
use cms_socket_db::{Msg, SOCK_FILE};
use std::{num::NonZeroUsize, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    runtime,
    signal::unix::{SignalKind, signal},
    sync, task, time,
};

#[derive(Parser, Debug, Clone)]
struct Opts {
    /// Path to the database directory.
    db_path: PathBuf,

    /// The run directory for runtime data.
    #[arg(long, default_value = "/run")]
    rundir: PathBuf,

    /// The number of elements held in the cache.
    #[arg(long, default_value = "1024")]
    cache_size: usize,

    /// Always run in non-systemd mode.
    #[arg(long, default_value = "false")]
    no_systemd: bool,

    /// Set the number async worker threads.
    #[arg(long, default_value = "3")]
    worker_threads: NonZeroUsize,

    /// Print debugging information.
    debug: bool,
}

async fn process_conn(mut conn: CmsSocketConn, db: Arc<DbCache>) -> ah::Result<()> {
    loop {
        let msg = conn.recv_msg(Msg::try_msg_deserialize).await?;
        match msg {
            Some(Msg::GetPage {
                path,
                get_title,
                get_data,
                get_stamp,
                get_redirect,
            }) => {
                let mut title = None;
                let mut data = None;
                let mut stamp = None;
                let mut redirect = None;

                if let Ok(path) = path.into_checked() {
                    if get_title {
                        title = Some(db.get_page_title(&path).await);
                    }
                    if get_data {
                        data = Some(db.get_page(&path).await);
                    }
                    if get_stamp {
                        stamp = Some(db.get_page_stamp(&path).await);
                    }
                    if get_redirect {
                        redirect = Some(db.get_page_redirect(&path).await);
                    }
                };

                let reply = Msg::Page {
                    title,
                    data,
                    stamp,
                    redirect,
                };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetHeaders { path }) => {
                let data;
                if let Ok(path) = path.into_checked() {
                    data = db.get_headers(&path).await;
                } else {
                    data = Default::default();
                }

                let reply = Msg::Headers { data };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetSubPages {
                path,
                get_nav_labels: _,
                get_nav_stops: _,
                get_stamps: _,
                get_prios: _,
            }) => {
                let mut names;
                let mut nav_labels;
                let mut nav_stops;
                let mut stamps;
                let mut prios;
                if let Ok(path) = path.into_checked() {
                    let mut infos = db.get_subpages(&path).await;
                    let count = infos.len();
                    names = Vec::with_capacity(count);
                    nav_labels = Vec::with_capacity(count);
                    nav_stops = Vec::with_capacity(count);
                    stamps = Vec::with_capacity(count);
                    prios = Vec::with_capacity(count);
                    for info in infos.drain(..) {
                        names.push(info.name);
                        nav_labels.push(info.nav_label);
                        nav_stops.push(info.nav_stop);
                        stamps.push(info.stamp);
                        prios.push(info.prio);
                    }
                } else {
                    names = vec![];
                    nav_labels = vec![];
                    nav_stops = vec![];
                    stamps = vec![];
                    prios = vec![];
                }

                let reply = Msg::SubPages {
                    names,
                    nav_labels,
                    nav_stops,
                    stamps,
                    prios,
                };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetMacro { parent, name }) => {
                let data;
                if let Ok(parent) = parent.into_checked() {
                    if let Ok(name) = name.into_checked_element() {
                        data = db.get_macro(&parent, &name).await;
                    } else {
                        data = Default::default();
                    }
                } else {
                    data = Default::default();
                }

                let reply = Msg::Macro { data };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetString { name }) => {
                let data;
                if let Ok(name) = name.into_checked_element() {
                    data = db.get_string(&name).await;
                } else {
                    data = Default::default();
                }

                let reply = Msg::String { data };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetImage { name }) => {
                //TODO: We should support a hierarchy of identifiers for images,
                //      just as we do for pages. In the db we should probably place
                //      these hierarchial images into the page directory.
                let data;
                if let Ok(name) = name.into_checked_element() {
                    data = db.get_image(&name).await;
                } else {
                    data = Default::default();
                }

                let reply = Msg::Image { data };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::Page { .. })
            | Some(Msg::Headers { .. })
            | Some(Msg::SubPages { .. })
            | Some(Msg::Macro { .. })
            | Some(Msg::String { .. })
            | Some(Msg::Image { .. }) => {
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

    let db = Arc::new(DbCache::new(DbFsIntf::new(&opts.db_path)?, opts.cache_size));

    let mut sock = CmsSocket::from_systemd_or_path(opts.no_systemd, &opts.rundir.join(SOCK_FILE))?;

    Filter::compile(
        &[
            Allow::Open,
            Allow::Read,
            Allow::Write,
            Allow::Stat,
            Allow::Listdir,
            Allow::Recv,
            Allow::Send,
            Allow::Futex,
            Allow::UnixListen,
            Allow::Signal,
            Allow::Mmap,
            Allow::Mprotect,
            Allow::Threading,
            Allow::Inotify,
            Allow::Prctl,
            Allow::Timer,
        ],
        Action::Kill,
    )
    .context("Compile seccomp filter")?
    .install()
    .context("Install seccomp filter")?;

    // Task: Socket handler.
    task::spawn({
        let db = Arc::clone(&db);
        async move {
            loop {
                match sock.accept().await {
                    Ok(conn) => {
                        // Socket connection handler.
                        let db = Arc::clone(&db);
                        task::spawn(async move {
                            if let Err(e) = process_conn(conn, db).await {
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
        }
    });

    // Task: Inotify handler.
    task::spawn({
        let db = Arc::clone(&db);
        async move {
            let mut interval = time::interval(Duration::from_millis(1000));
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                db.check_inotify().await;
            }
        }
    });

    // Task: Debugging.
    if opts.debug {
        task::spawn({
            let db = Arc::clone(&db);
            async move {
                let mut interval = time::interval(Duration::from_millis(10000));
                interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
                loop {
                    interval.tick().await;
                    db.print_debug().await;
                }
            }
        });
    }

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
                db.clear().await;
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

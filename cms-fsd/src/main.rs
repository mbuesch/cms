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
use anyhow::{self as ah, format_err as err, Context as _};
use clap::Parser;
use cms_seccomp::{seccomp_compile, seccomp_install, Action, Allow};
use cms_socket::{CmsSocket, CmsSocketConn, MsgSerde};
use cms_socket_db::{Msg, SOCK_FILE};
use std::{num::NonZeroUsize, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    runtime,
    signal::unix::{signal, SignalKind},
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
                get_prio,
                get_redirect,
                get_nav_stop,
                get_nav_label,
            }) => {
                let mut title = None;
                let mut data = None;
                let mut stamp = None;
                let mut prio = None;
                let mut redirect = None;
                let mut nav_stop = None;
                let mut nav_label = None;

                //TODO: Cleaning should be done in the backd.
                if let Ok(path) = path.into_cleaned_path().into_checked() {
                    if get_title {
                        title = Some(db.get_page_title(&path).await);
                    }
                    if get_data {
                        data = Some(db.get_page(&path).await);
                    }
                    if get_stamp {
                        stamp = Some(db.get_page_stamp(&path).await);
                    }
                    if get_prio {
                        prio = Some(db.get_page_prio(&path).await);
                    }
                    if get_redirect {
                        redirect = Some(db.get_page_redirect(&path).await);
                    }
                    if get_nav_stop {
                        nav_stop = Some(db.get_nav_stop(&path).await);
                    }
                    if get_nav_label {
                        nav_label = Some(db.get_nav_label(&path).await);
                    }
                };

                let reply = Msg::Page {
                    title,
                    data,
                    stamp,
                    prio,
                    redirect,
                    nav_stop,
                    nav_label,
                };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetHeaders { path }) => {
                //TODO: Cleaning should be done in the backd.
                let data;
                if let Ok(path) = path.into_cleaned_path().into_checked() {
                    data = db.get_headers(&path).await;
                } else {
                    data = Default::default();
                }

                let reply = Msg::Headers { data };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetSubPages { path }) => {
                //TODO: Cleaning should be done in the backd.
                let mut names;
                let mut nav_labels;
                let mut prios;
                if let Ok(path) = path.into_cleaned_path().into_checked() {
                    let mut infos = db.get_subpages(&path).await;
                    names = Vec::with_capacity(infos.len());
                    nav_labels = Vec::with_capacity(infos.len());
                    prios = Vec::with_capacity(infos.len());
                    for info in infos.drain(..) {
                        names.push(info.name);
                        nav_labels.push(info.nav_label);
                        prios.push(info.prio);
                    }
                } else {
                    names = vec![];
                    nav_labels = vec![];
                    prios = vec![];
                }

                let reply = Msg::SubPages {
                    names,
                    nav_labels,
                    prios,
                };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetMacro { parent, name }) => {
                //TODO: Cleaning should be done in the backd.
                let data;
                if let Ok(parent) = parent.into_cleaned_path().into_checked() {
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

    seccomp_install(
        seccomp_compile(
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
                Allow::SignalReturn,
                Allow::SignalMask,
                Allow::Mmap,
                Allow::Mprotect,
                Allow::Threading,
                Allow::Inotify,
                Allow::Prctl,
                Allow::Timer,
                Allow::ClockGet,
                Allow::Sleep,
            ],
            Action::Kill,
        )
        .context("Compile seccomp filter")?,
    )
    .context("Install seccomp filter")?;

    // Task: Socket handler.
    let db_clone = Arc::clone(&db);
    task::spawn(async move {
        loop {
            match sock.accept().await {
                Ok(conn) => {
                    // Socket connection handler.
                    let db_clone = Arc::clone(&db_clone);
                    task::spawn(async move {
                        if let Err(e) = process_conn(conn, db_clone).await {
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

    // Task: Inotify handler.
    let db_clone = Arc::clone(&db);
    task::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(1000));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            db_clone.check_inotify().await;
        }
    });

    // Main task.
    let db_clone = Arc::clone(&db);
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
                db_clone.clear().await;
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

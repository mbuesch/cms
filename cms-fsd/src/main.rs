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

mod db_cache;
mod db_fsintf;

use crate::{db_cache::DbCache, db_fsintf::DbFsIntf};
use anyhow::{self as ah, format_err as err, Context as _};
use clap::Parser;
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
    #[arg(long, default_value = "1024", value_parser = clap::value_parser!(u32).range(1..))]
    cache_size: u32,

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
                let path = path.into_cleaned_path().into_checked()?;

                let title = if get_title {
                    Some(db.get_page_title(&path).await)
                } else {
                    None
                };
                let data = if get_data {
                    Some(db.get_page(&path).await)
                } else {
                    None
                };
                let stamp = if get_stamp {
                    Some(db.get_page_stamp(&path).await)
                } else {
                    None
                };
                let prio = if get_prio {
                    Some(db.get_page_prio(&path).await)
                } else {
                    None
                };
                let redirect = if get_redirect {
                    Some(db.get_page_redirect(&path).await)
                } else {
                    None
                };
                let nav_stop = if get_nav_stop {
                    Some(db.get_nav_stop(&path).await)
                } else {
                    None
                };
                let nav_label = if get_nav_label {
                    Some(db.get_nav_label(&path).await)
                } else {
                    None
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
                let path = path.into_cleaned_path().into_checked()?;

                let data = db.get_headers(&path).await;

                let reply = Msg::Headers { data };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetSubPages { path }) => {
                let path = path.into_cleaned_path().into_checked()?;

                let mut infos = db.get_subpages(&path).await;

                let mut names = Vec::with_capacity(infos.len());
                let mut nav_labels = Vec::with_capacity(infos.len());
                let mut prios = Vec::with_capacity(infos.len());
                for info in infos.drain(..) {
                    names.push(info.name);
                    nav_labels.push(info.nav_label);
                    prios.push(info.prio);
                }

                let reply = Msg::SubPages {
                    names,
                    nav_labels,
                    prios,
                };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetMacro { parent, name }) => {
                let parent = parent.into_cleaned_path().into_checked()?;
                let name = name.into_checked_element()?;

                let data = db.get_macro(&parent, &name).await;

                let reply = Msg::Macro { data };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetString { name }) => {
                let name = name.into_checked_element()?;

                let data = db.get_string(&name).await;

                let reply = Msg::String { data };
                conn.send_msg(&reply).await?;
            }
            Some(Msg::GetImage { name }) => {
                let name = name.into_checked_element()?;

                let data = db.get_image(&name).await;

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
        .thread_keep_alive(Duration::from_millis(0))
        .worker_threads(opts.worker_threads.into())
        .enable_all()
        .build()
        .context("Tokio runtime builder")?
        .block_on(async_main(opts))
}

// vim: ts=4 sw=4 expandtab

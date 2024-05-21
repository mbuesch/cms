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

mod anchor;
mod args;
mod backend;
mod cache;
mod comm;
mod config;
mod cookie;
mod formfields;
mod index;
mod itertools;
mod navtree;
mod numparse;
mod pagegen;
mod query;
mod reply;
mod resolver;
mod sitemap;

use crate::{
    args::{CmsGetArgs, CmsPostArgs},
    backend::CmsBack,
    cache::CmsCache,
    config::CmsConfig,
    cookie::Cookie,
    query::Query,
    reply::CmsReply,
};
use anyhow::{self as ah, format_err as err, Context as _};
use clap::Parser;
use cms_socket::{CmsSocket, CmsSocketConn, MsgSerde};
use cms_socket_back::{Msg, SOCK_FILE};
use std::{
    num::NonZeroUsize,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
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

async fn process_conn(
    mut conn: CmsSocketConn,
    config: Arc<CmsConfig>,
    cache: Arc<CmsCache>,
    opts: Arc<Opts>,
) -> ah::Result<()> {
    let mut back = CmsBack::new(Arc::clone(&config), cache, &opts.rundir).await;
    loop {
        let msg = conn.recv_msg(Msg::try_msg_deserialize).await?;

        let start_stamp = if config.debug() {
            Some(Instant::now())
        } else {
            None
        };

        let mut reply: CmsReply = match msg {
            Some(Msg::Get {
                host,
                path,
                https,
                cookie,
                query,
            }) => {
                let path = path.into_cleaned_path().into_checked_sys()?;

                back.get(&CmsGetArgs {
                    host,
                    path,
                    _cookie: Cookie::new(cookie),
                    query: Query::new(query),
                    https,
                })
                .await
            }
            Some(Msg::Post {
                host,
                path,
                https,
                cookie,
                query,
                body,
                body_mime,
            }) => {
                let path = path.into_cleaned_path().into_checked()?;

                back.post(
                    &CmsGetArgs {
                        host,
                        path,
                        _cookie: Cookie::new(cookie),
                        query: Query::new(query),
                        https,
                    },
                    &CmsPostArgs { body, body_mime },
                )
                .await
            }
            Some(Msg::Reply { .. }) => {
                eprintln!("Received unsupported message.");
                continue;
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("Client disconnected.");
                return Ok(());
            }
        };

        if let Some(start_stamp) = start_stamp {
            let runtime = (Instant::now() - start_stamp).as_micros();
            reply.add_http_header(&format!("X-CMS-Backend-Runtime: {runtime} us"));
        }

        let reply_msg: Msg = reply.into();
        conn.send_msg(&reply_msg).await?;
    }
}

async fn async_main(opts: Arc<Opts>) -> ah::Result<()> {
    let (main_exit_tx, mut main_exit_rx) = sync::mpsc::channel(1);

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sighup = signal(SignalKind::hangup()).unwrap();

    let mut sock = CmsSocket::from_systemd_or_path(opts.no_systemd, &opts.rundir.join(SOCK_FILE))?;
    let config = Arc::new(CmsConfig::new().context("backd.conf")?);

    //TODO install seccomp filter.

    let cache = Arc::new(CmsCache::new(opts.cache_size));

    // Task: Socket handler.
    let config_sock = Arc::clone(&config);
    let cache_sock = Arc::clone(&cache);
    let opts_sock = Arc::clone(&opts);
    task::spawn(async move {
        loop {
            let config = Arc::clone(&config_sock);
            let cache = Arc::clone(&cache_sock);
            let opts = Arc::clone(&opts_sock);
            match sock.accept().await {
                Ok(conn) => {
                    // Socket connection handler.
                    task::spawn(async move {
                        if let Err(e) = process_conn(conn, config, cache, opts).await {
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
    let cache_main = Arc::clone(&cache);
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
                cache_main.clear().await;
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

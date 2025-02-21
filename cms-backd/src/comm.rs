// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::{self as ah, Context as _, format_err as err};
use chrono::prelude::*;
use cms_ident::{CheckedIdent, CheckedIdentElem, Tail};
use cms_socket::{CmsSocketConn, MsgSerde as _};
use cms_socket_db::{Msg as MsgDb, SOCK_FILE as SOCK_FILE_DB};
use cms_socket_post::{Msg as MsgPost, SOCK_FILE as SOCK_FILE_POST};
use lru::LruCache;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const DEBUG: bool = false;
const MACRO_CACHE_SIZE: usize = 512;

fn epoch_stamp(seconds: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds.try_into().unwrap_or_default(), 0).unwrap_or_default()
}

#[derive(Clone, Debug, Default)]
pub struct CommGetPage {
    pub path: CheckedIdent,
    pub get_title: bool,
    pub get_data: bool,
    pub get_stamp: bool,
    pub get_redirect: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CommPage {
    pub title: Option<String>,
    pub data: Option<String>,
    pub stamp: Option<DateTime<Utc>>,
    pub redirect: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CommSubPages {
    pub names: Vec<String>,
    pub nav_labels: Vec<String>,
    pub nav_stops: Vec<bool>,
    pub stamps: Vec<DateTime<Utc>>,
    pub prios: Vec<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct CommRunPostHandler {
    pub path: CheckedIdent,
    pub query: HashMap<String, Vec<u8>>,
    pub form_fields: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Default)]
pub struct CommPostHandlerResult {
    pub error: String,
    pub body: Vec<u8>,
    pub mime: String,
}

/// Communication with database and post handler.
pub struct CmsComm {
    sock_path_db: PathBuf,
    sock_path_post: PathBuf,
    sock_db: Option<CmsSocketConn>,
    sock_post: Option<CmsSocketConn>,
    macro_cache: LruCache<String, String>,
}

impl CmsComm {
    pub fn new(rundir: &Path) -> Self {
        let sock_path_db = rundir.join(SOCK_FILE_DB);
        let sock_path_post = rundir.join(SOCK_FILE_POST);
        Self {
            sock_path_db,
            sock_path_post,
            sock_db: None,
            sock_post: None,
            macro_cache: LruCache::new(MACRO_CACHE_SIZE.try_into().unwrap()),
        }
    }

    async fn sock_db(&mut self) -> ah::Result<&mut CmsSocketConn> {
        if self.sock_db.is_none() {
            self.sock_db = Some(CmsSocketConn::connect(&self.sock_path_db).await?);
        }
        Ok(self.sock_db.as_mut().unwrap())
    }

    async fn sock_post(&mut self) -> ah::Result<&mut CmsSocketConn> {
        if self.sock_post.is_none() {
            self.sock_post = Some(CmsSocketConn::connect(&self.sock_path_post).await?);
        }
        Ok(self.sock_post.as_mut().unwrap())
    }

    async fn comm_db(&mut self, request: &MsgDb) -> ah::Result<MsgDb> {
        if DEBUG {
            println!("DB comm: {request:?}");
        }
        let sock = self.sock_db().await?;
        sock.send_msg(request).await?;
        if let Some(reply) = sock.recv_msg(MsgDb::try_msg_deserialize).await? {
            Ok(reply)
        } else {
            Err(err!("cms-fsd disconnected"))
        }
    }

    async fn comm_post(&mut self, request: &MsgPost) -> ah::Result<MsgPost> {
        if DEBUG {
            println!("Post comm: {request:?}");
        }
        let sock = self.sock_post().await?;
        sock.send_msg(request).await?;
        if let Some(reply) = sock.recv_msg(MsgPost::try_msg_deserialize).await? {
            Ok(reply)
        } else {
            Err(err!("cms-postd disconnected"))
        }
    }

    pub async fn get_db_page(&mut self, get: CommGetPage) -> ah::Result<CommPage> {
        let reply = self
            .comm_db(&MsgDb::GetPage {
                path: get.path.downgrade_clone(),
                get_title: get.get_title,
                get_data: get.get_data,
                get_stamp: get.get_stamp,
                get_redirect: get.get_redirect,
            })
            .await;
        if let Ok(MsgDb::Page {
            title,
            data,
            stamp,
            redirect,
        }) = reply
        {
            Ok(CommPage {
                title: title.and_then(|x| String::from_utf8(x).ok()),
                data: data.and_then(|x| String::from_utf8(x).ok()),
                stamp: stamp.map(epoch_stamp),
                redirect: redirect.and_then(|x| String::from_utf8(x).ok()),
            })
        } else {
            Err(err!("Page: Invalid db reply."))
        }
    }

    pub async fn get_db_sub_pages(&mut self, path: &CheckedIdent) -> ah::Result<CommSubPages> {
        let reply = self
            .comm_db(&MsgDb::GetSubPages {
                path: path.downgrade_clone(),
                get_nav_labels: true,
                get_nav_stops: true,
                get_stamps: true,
                get_prios: true,
            })
            .await;
        if let Ok(MsgDb::SubPages {
            names,
            nav_labels,
            nav_stops,
            stamps,
            prios,
        }) = reply
        {
            let count = names.len();
            if nav_labels.len() == count
                && nav_stops.len() == count
                && stamps.len() == count
                && prios.len() == count
            {
                Ok(CommSubPages {
                    names: names
                        .into_iter()
                        .map(|x| String::from_utf8(x).unwrap_or_default())
                        .collect(),
                    nav_labels: nav_labels
                        .into_iter()
                        .map(|x| String::from_utf8(x).unwrap_or_default())
                        .collect(),
                    nav_stops,
                    stamps: stamps.into_iter().map(epoch_stamp).collect(),
                    prios,
                })
            } else {
                Err(err!("GetSubPages: Invalid db reply (length)."))
            }
        } else {
            Err(err!("GetSubPages: Invalid db reply."))
        }
    }

    pub async fn get_db_headers(&mut self, path: &CheckedIdent) -> ah::Result<String> {
        let reply = self
            .comm_db(&MsgDb::GetHeaders {
                path: path.downgrade_clone(),
            })
            .await;
        if let Ok(MsgDb::Headers { data }) = reply {
            Ok(String::from_utf8(data).context("Headers: Data is not valid UTF-8")?)
        } else {
            Err(err!("Headers: Invalid db reply."))
        }
    }

    pub async fn get_db_string(&mut self, name: &str) -> ah::Result<String> {
        let reply = self
            .comm_db(&MsgDb::GetString {
                name: name.parse().context("Invalid DB string name")?,
            })
            .await;
        if let Ok(MsgDb::String { data }) = reply {
            Ok(String::from_utf8(data).context("String: Data is not valid UTF-8")?)
        } else {
            Err(err!("String: Invalid db reply."))
        }
    }

    pub async fn get_db_macro(
        &mut self,
        parent: Option<&CheckedIdent>,
        name: &CheckedIdentElem,
    ) -> ah::Result<String> {
        let cache_name = if let Some(parent) = parent {
            parent.to_fs_path(Path::new(""), &Tail::One(name.clone()))
        } else {
            name.to_fs_path(Path::new(""), &Tail::None)
        };
        let cache_name = cache_name.into_os_string().into_string().unwrap();

        // Try to get it from the cache.
        if let Some(data) = self.macro_cache.get(&cache_name) {
            return Ok(data.clone());
        }

        let reply = self
            .comm_db(&MsgDb::GetMacro {
                parent: parent.unwrap_or(&CheckedIdent::ROOT).downgrade_clone(),
                name: name.downgrade_clone(),
            })
            .await;
        if let Ok(MsgDb::Macro { data }) = reply {
            let data = String::from_utf8(data).context("Macro: Data is not valid UTF-8")?;

            // Put it into the cache.
            self.macro_cache.push(cache_name, data.clone());
            Ok(data)
        } else {
            Err(err!("Macro: Invalid db reply."))
        }
    }

    pub async fn get_db_image(&mut self, name: &CheckedIdentElem) -> ah::Result<Vec<u8>> {
        let reply = self
            .comm_db(&MsgDb::GetImage {
                name: name.downgrade_clone(),
            })
            .await;
        if let Ok(MsgDb::Image { data }) = reply {
            Ok(data)
        } else {
            Err(err!("Image: Invalid db reply."))
        }
    }

    pub async fn run_post_handler(
        &mut self,
        run: CommRunPostHandler,
    ) -> ah::Result<CommPostHandlerResult> {
        let reply = self
            .comm_post(&MsgPost::RunPostHandler {
                path: run.path.downgrade_clone(),
                query: run.query,
                form_fields: run.form_fields,
            })
            .await;
        if let Ok(MsgPost::PostHandlerResult { error, body, mime }) = reply {
            Ok(CommPostHandlerResult { error, body, mime })
        } else {
            Err(err!("RunPostHandler: Invalid postd reply."))
        }
    }
}

// vim: ts=4 sw=4 expandtab

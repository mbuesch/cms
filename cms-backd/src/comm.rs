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

use anyhow::{self as ah, format_err as err, Context as _};
use chrono::prelude::*;
use cms_ident::{CheckedIdent, CheckedIdentElem};
use cms_socket::{CmsSocketConn, MsgSerde as _};
use cms_socket_db::{Msg as MsgDb, SOCK_FILE as SOCK_FILE_DB};
use cms_socket_post::{Msg as MsgPost, SOCK_FILE as SOCK_FILE_POST};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn epoch_stamp(seconds: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds.try_into().unwrap_or_default(), 0).unwrap_or_default()
}

#[derive(Clone, Debug, Default)]
pub struct CommGetPage {
    pub path: CheckedIdent,
    pub get_title: bool,
    pub get_data: bool,
    pub get_stamp: bool,
    pub get_prio: bool,
    pub get_redirect: bool,
    pub get_nav_stop: bool,
    pub get_nav_label: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CommPage {
    pub title: Option<String>,
    pub data: Option<String>,
    pub stamp: Option<DateTime<Utc>>,
    pub prio: Option<u64>,
    pub redirect: Option<String>,
    pub nav_stop: Option<bool>,
    pub nav_label: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CommSubPages {
    pub names: Vec<String>,
    pub nav_labels: Vec<String>,
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
        let sock = self.sock_db().await?;
        sock.send_msg(request).await?;
        if let Some(reply) = sock.recv_msg(MsgDb::try_msg_deserialize).await? {
            Ok(reply)
        } else {
            Err(err!("cms-fsd disconnected"))
        }
    }

    async fn comm_post(&mut self, request: &MsgPost) -> ah::Result<MsgPost> {
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
                get_prio: get.get_prio,
                get_redirect: get.get_redirect,
                get_nav_stop: get.get_nav_stop,
                get_nav_label: get.get_nav_label,
            })
            .await;
        if let Ok(MsgDb::Page {
            title,
            data,
            stamp,
            prio,
            redirect,
            nav_stop,
            nav_label,
        }) = reply
        {
            Ok(CommPage {
                title: title.and_then(|x| String::from_utf8(x).ok()),
                data: data.and_then(|x| String::from_utf8(x).ok()),
                stamp: stamp.map(epoch_stamp),
                prio,
                redirect: redirect.and_then(|x| String::from_utf8(x).ok()),
                nav_stop,
                nav_label: nav_label.and_then(|x| String::from_utf8(x).ok()),
            })
        } else {
            Err(err!("Page: Invalid db reply."))
        }
    }

    pub async fn get_db_sub_pages(&mut self, path: &CheckedIdent) -> ah::Result<CommSubPages> {
        let reply = self
            .comm_db(&MsgDb::GetSubPages {
                path: path.downgrade_clone(),
            })
            .await;
        if let Ok(MsgDb::SubPages {
            names,
            nav_labels,
            prios,
        }) = reply
        {
            if names.len() == nav_labels.len() && names.len() == prios.len() {
                Ok(CommSubPages {
                    names: names
                        .into_iter()
                        .map(|x| String::from_utf8(x).unwrap_or_default())
                        .collect(),
                    nav_labels: nav_labels
                        .into_iter()
                        .map(|x| String::from_utf8(x).unwrap_or_default())
                        .collect(),
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
        let reply = self
            .comm_db(&MsgDb::GetMacro {
                parent: parent.unwrap_or(&CheckedIdent::ROOT).downgrade_clone(),
                name: name.downgrade_clone(),
            })
            .await;
        if let Ok(MsgDb::Macro { data }) = reply {
            Ok(String::from_utf8(data).context("Macro: Data is not valid UTF-8")?)
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

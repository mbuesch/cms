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
use cms_ident::{CheckedIdent, CheckedIdentElem};
use cms_socket::{CmsSocketConn, MsgSerde as _};
use cms_socket_db::{Msg as MsgDb, SOCK_FILE as SOCK_FILE_DB};
use cms_socket_post::{Msg as MsgPost, SOCK_FILE as SOCK_FILE_POST};
use std::path::{Path, PathBuf};

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

    pub async fn sock_db(&mut self) -> ah::Result<&mut CmsSocketConn> {
        if self.sock_db.is_none() {
            self.sock_db = Some(CmsSocketConn::connect(&self.sock_path_db).await?);
        }
        Ok(self.sock_db.as_mut().unwrap())
    }

    pub async fn sock_post(&mut self) -> ah::Result<&mut CmsSocketConn> {
        if self.sock_post.is_none() {
            self.sock_post = Some(CmsSocketConn::connect(&self.sock_path_post).await?);
        }
        Ok(self.sock_post.as_mut().unwrap())
    }

    pub async fn comm_db(&mut self, request: &MsgDb) -> ah::Result<MsgDb> {
        let sock = self.sock_db().await?;
        sock.send_msg(request).await?;
        if let Some(reply) = sock.recv_msg(MsgDb::try_msg_deserialize).await? {
            Ok(reply)
        } else {
            Err(err!("cms-fsd disconnected"))
        }
    }

    pub async fn comm_post(&mut self, request: &MsgPost) -> ah::Result<MsgPost> {
        let sock = self.sock_post().await?;
        sock.send_msg(request).await?;
        if let Some(reply) = sock.recv_msg(MsgPost::try_msg_deserialize).await? {
            Ok(reply)
        } else {
            Err(err!("cms-postd disconnected"))
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
}

// vim: ts=4 sw=4 expandtab

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

use crate::{
    args::{get_query_var, CmsGetArgs, CmsPostArgs},
    cache::CmsCache,
    config::CmsConfig,
    navtree::NavTree,
    pagegen::PageGen,
    reply::{result_to_reply, CmsReply, HttpStatus},
    resolver::{getvar, Resolver, ResolverVars},
};
use anyhow::{self as ah, format_err as err, Context as _};
use chrono::prelude::*;
use cms_ident::{CheckedIdent, CheckedIdentElem, UrlComp};
use cms_socket::{CmsSocketConn, MsgSerde as _};
use cms_socket_db::{Msg as MsgDb, SOCK_FILE as SOCK_FILE_DB};
use cms_socket_post::{Msg as MsgPost, SOCK_FILE as SOCK_FILE_POST};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

fn epoch_stamp(seconds: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds.try_into().unwrap_or_default(), 0).unwrap_or_default()
}

/// Communication with database and post handler.
pub struct CmsComm {
    sock_path_db: PathBuf,
    sock_path_post: PathBuf,
    sock_db: Option<CmsSocketConn>,
    sock_post: Option<CmsSocketConn>,
}

impl CmsComm {
    fn new(rundir: &Path) -> Self {
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
                parent: parent.unwrap_or(&CheckedIdent::ROOT).clone().downgrade(),
                name: name.clone().downgrade(),
            })
            .await;
        if let Ok(MsgDb::Macro { data }) = reply {
            Ok(String::from_utf8(data).context("Macro: Data is not valid UTF-8")?)
        } else {
            Err(err!("Macro: Invalid db reply."))
        }
    }
}

macro_rules! resolve {
    ($comm:expr, $get:expr, $config:expr, $vars:expr, $debug:expr, $text:expr) => {
        Resolver::new(
            &mut $comm,
            $get,
            Arc::clone(&$config),
            &$get.path,
            &$vars,
            $debug,
        )
        .run(&$text)
        .await?
    };
}

pub struct CmsBack {
    config: Arc<CmsConfig>,
    #[allow(dead_code)] //TODO
    cache: Arc<CmsCache>,
    comm: CmsComm,
}

impl CmsBack {
    pub async fn new(config: Arc<CmsConfig>, cache: Arc<CmsCache>, rundir: &Path) -> Self {
        Self {
            config,
            cache,
            comm: CmsComm::new(rundir),
        }
    }

    async fn get_page(&mut self, get: &CmsGetArgs) -> ah::Result<CmsReply> {
        let debug = true; //TODO

        let reply = self
            .comm
            .comm_db(&MsgDb::GetPage {
                path: get.path.downgrade_clone(),
                get_title: true,
                get_data: true,
                get_stamp: true,
                get_prio: false,
                get_redirect: true,
                get_nav_stop: false,
                get_nav_label: false,
            })
            .await;
        let Ok(MsgDb::Page {
            title,
            data,
            stamp,
            redirect,
            ..
        }) = reply
        else {
            return Ok(CmsReply::internal_error("GetPage: Invalid db reply"));
        };

        let redirect = String::from_utf8(redirect.unwrap_or_default()).unwrap_or_default();
        if !redirect.is_empty() {
            return Ok(CmsReply::redirect(&redirect));
        }

        let mut title = String::from_utf8(title.unwrap_or_default()).unwrap_or_default();
        let mut data = String::from_utf8(data.unwrap_or_default()).unwrap_or_default();
        let stamp = epoch_stamp(stamp.unwrap_or_default());

        let reply = self
            .comm
            .comm_db(&MsgDb::GetHeaders {
                path: get.path.downgrade_clone(),
            })
            .await;
        let Ok(MsgDb::Headers { data: headers }) = reply else {
            return Ok(CmsReply::internal_error("GetHeaders: Invalid db reply"));
        };
        let mut headers = String::from_utf8(headers).unwrap_or_default();

        let navtree = NavTree::build(&mut self.comm, &CheckedIdent::ROOT, Some(&get.path)).await;

        let mut homestr = self.comm.get_db_string("home").await.unwrap_or_default();

        let mut vars = ResolverVars::new();
        vars.register("PROTOCOL", getvar!(get.protocol_str().to_string()));
        vars.register(
            "PAGEIDENT",
            getvar!(get.path.url(UrlComp {
                protocol: None,
                domain: None,
                base: None,
            })),
        );
        vars.register(
            "CMS_PAGEIDENT",
            getvar!(get.path.url(UrlComp {
                protocol: None,
                domain: None,
                base: Some(self.config.url_base()),
            })),
        );
        vars.register(
            "GROUP",
            getvar!(get.path.nth_element_str(0).unwrap_or("").to_string()),
        );
        vars.register(
            "PAGE",
            getvar!(get.path.nth_element_str(1).unwrap_or("").to_string()),
        );
        vars.register_prefix("Q", Arc::new(|name| get_query_var(get, name, true)));
        vars.register_prefix("QRAW", Arc::new(|name| get_query_var(get, name, false)));

        title = resolve!(self.comm, get, self.config, vars, debug, title);

        vars.register("TITLE", getvar!(title.clone()));

        data = resolve!(self.comm, get, self.config, vars, debug, data);
        headers = resolve!(self.comm, get, self.config, vars, debug, headers);
        homestr = resolve!(self.comm, get, self.config, vars, debug, homestr);

        let now = Utc::now();

        Ok(PageGen::new(get, Arc::clone(&self.config))
            .generate(&title, &headers, &data, &now, &stamp, &navtree, &homestr))
    }

    async fn get_image(&mut self, get: &CmsGetArgs, thumb: bool) -> CmsReply {
        //TODO
        Default::default()
    }

    async fn get_sitemap(&mut self, get: &CmsGetArgs) -> CmsReply {
        //TODO
        Default::default()
    }

    async fn get_css(&mut self, get: &CmsGetArgs) -> CmsReply {
        if let Some(css_name) = get.path.nth_element_str(1) {
            if css_name == "cms.css" {
                return result_to_reply!(
                    self.comm.get_db_string("css").await.map(|s| s.into_bytes()),
                    "text/css; charset=UTF-8",
                    not_found
                );
            }
        }
        CmsReply::not_found("Invalid CSS name")
    }

    pub async fn get(&mut self, get: &CmsGetArgs) -> CmsReply {
        let count = get.path.element_count();
        let first = get.path.first_element_str();
        let reply = match first {
            Some("__thumbs") if count == 2 => self.get_image(get, true).await,
            Some("__images") if count == 2 => self.get_image(get, false).await,
            Some("__sitemap") | Some("__sitemap.xml") if count == 1 => self.get_sitemap(get).await,
            Some("__css") if count == 2 => self.get_css(get).await,
            _ => self.get_page(get).await.into(),
        };
        if reply.status() == HttpStatus::InternalServerError {
            //TODO reduce information, if not debugging
        }
        reply
    }

    pub async fn post(&mut self, get: &CmsGetArgs, post: &CmsPostArgs) -> CmsReply {
        //TODO
        Default::default()
    }
}

// vim: ts=4 sw=4 expandtab

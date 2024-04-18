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

use crate::{cache::CmsCache, cookie::Cookie, query::Query};
use cms_ident::CheckedIdent;
use std::sync::Arc;

pub struct CmsGetArgs {
    pub host: String,
    pub path: CheckedIdent,
    pub _cookie: Cookie,
    pub query: Query,
    pub https: bool,
}

pub struct CmsPostArgs {
    pub body: Vec<u8>,
    pub body_mime: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u32)]
pub enum HttpStatus {
    Ok = 200,
    BadRequest = 400,
    NotFound = 404,
    #[default]
    InternalServerError = 500,
}

impl From<HttpStatus> for u32 {
    fn from(status: HttpStatus) -> Self {
        status as Self
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CmsReply {
    status: HttpStatus,
    body: Vec<u8>,
    mime: String,
    extra_headers: Vec<String>,
}

impl CmsReply {
    fn new() -> Self {
        Default::default()
    }
}

impl From<CmsReply> for cms_socket_back::Msg {
    fn from(reply: CmsReply) -> Self {
        cms_socket_back::Msg::Reply {
            status: reply.status.into(),
            body: reply.body,
            mime: reply.mime,
            extra_headers: reply.extra_headers,
        }
    }
}

pub struct CmsBack {
    cache: Arc<CmsCache>,
}

impl CmsBack {
    pub fn new(cache: Arc<CmsCache>) -> Self {
        Self { cache }
    }

    pub fn get(&mut self, get: &CmsGetArgs) -> CmsReply {
        //TODO
        CmsReply::new()
    }

    pub fn post(&mut self, get: &CmsGetArgs, post: &CmsPostArgs) -> CmsReply {
        //TODO
        CmsReply::new()
    }
}

// vim: ts=4 sw=4 expandtab

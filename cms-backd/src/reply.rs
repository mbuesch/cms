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

use anyhow as ah;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HttpStatus {
    Ok = 200,
    MovedPermanently = 301,
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

#[derive(Clone, Debug, Default)]
pub struct CmsReply {
    status: HttpStatus,
    body: Vec<u8>,
    mime: String,
    extra_http_headers: Vec<String>,
    extra_html_headers: Vec<String>,
}

impl CmsReply {
    pub fn ok(body: Vec<u8>, mime: String) -> Self {
        Self {
            status: HttpStatus::Ok,
            body,
            mime,
            ..Default::default()
        }
    }

    pub fn not_found(_msg: &str) -> Self {
        //TODO msg
        Self {
            status: HttpStatus::NotFound,
            ..Default::default()
        }
    }

    pub fn redirect(location: &str) -> Self {
        Self {
            status: HttpStatus::MovedPermanently,
            body: format!(
                r#"<p style="font-size: large;">Moved permanently to <a href="{location}">{location}</a></p>"#
            )
            .into_bytes(),
            mime: "text/html".to_string(),
            extra_http_headers: vec![format!(r#"Location: {location}"#)],
            extra_html_headers: vec![format!(
                r#"<meta http-equiv="refresh" content="0; URL={location}" />"#
            )],
        }
    }

    pub fn internal_error(_msg: &str) -> Self {
        //TODO msg
        Self {
            status: HttpStatus::InternalServerError,
            ..Default::default()
        }
    }

    pub fn status(&self) -> HttpStatus {
        self.status
    }
}

impl From<ah::Result<CmsReply>> for CmsReply {
    fn from(reply: ah::Result<CmsReply>) -> Self {
        match reply {
            Ok(reply) => reply,
            Err(err) => Self::internal_error(&format!("{err}")),
        }
    }
}

impl From<CmsReply> for cms_socket_back::Msg {
    fn from(reply: CmsReply) -> Self {
        cms_socket_back::Msg::Reply {
            status: reply.status.into(),
            body: reply.body,
            mime: reply.mime,
            extra_headers: reply.extra_http_headers,
        }
    }
}

macro_rules! result_to_reply {
    ($result:expr, $mime:expr, $err_ctor:ident) => {
        match $result {
            Err(e) => CmsReply::$err_ctor(&format!("{e}")),
            Ok(body) => CmsReply::ok(body, $mime.to_string()),
        }
    };
}
pub(crate) use result_to_reply;

// vim: ts=4 sw=4 expandtab

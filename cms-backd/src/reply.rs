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
use std::fmt;

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

impl fmt::Display for HttpStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let text = match self {
            Self::Ok => "Ok",
            Self::MovedPermanently => "Moved Permanently",
            Self::BadRequest => "Bad Request",
            Self::NotFound => "Not Found",
            Self::InternalServerError => "Internal Server Error",
        };
        write!(f, "{} {}", *self as u16, text)
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
    pub fn ok(body: Vec<u8>, mime: &str) -> Self {
        Self {
            status: HttpStatus::Ok,
            body,
            mime: mime.to_string(),
            ..Default::default()
        }
    }

    pub fn not_found(msg: &str) -> Self {
        Self {
            status: HttpStatus::NotFound,
            body: format!("<h1>{}: {}</h1>", HttpStatus::NotFound, msg).into_bytes(),
            mime: "text/html".to_string(),
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

    pub fn internal_error(msg: &str) -> Self {
        Self {
            status: HttpStatus::InternalServerError,
            body: format!("<h1>{}: {}</h1>", HttpStatus::InternalServerError, msg).into_bytes(),
            mime: "text/html".to_string(),
            ..Default::default()
        }
    }

    pub fn status(&self) -> HttpStatus {
        self.status
    }

    pub fn is_ok(&self) -> bool {
        self.status() == HttpStatus::Ok
    }

    pub fn set_status_as_body(&mut self) {
        self.body = format!("<h1>{}</h1>", self.status).into_bytes();
        self.mime = "text/html".to_string();
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

// vim: ts=4 sw=4 expandtab

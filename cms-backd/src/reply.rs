// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
    error_msg: String,
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
            body: format!(
                r#"<p style="font-size: large;">{}: {}</p>"#,
                HttpStatus::NotFound,
                msg
            )
            .into_bytes(),
            mime: "text/html".to_string(),
            error_msg: msg.to_string(),
            ..Default::default()
        }
    }

    pub fn bad_request(msg: &str) -> Self {
        Self {
            status: HttpStatus::BadRequest,
            body: format!(
                r#"<p style="font-size: large;">{}: {}</p>"#,
                HttpStatus::BadRequest,
                msg
            )
            .into_bytes(),
            mime: "text/html".to_string(),
            error_msg: msg.to_string(),
            ..Default::default()
        }
    }

    pub fn redirect(location: &str) -> Self {
        let location = location.trim();
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
            ..Default::default()
        }
    }

    pub fn internal_error(msg: &str) -> Self {
        Self {
            status: HttpStatus::InternalServerError,
            body: format!(
                r#"<p style="font-size: large;">{}: {}</p>"#,
                HttpStatus::InternalServerError,
                msg
            )
            .into_bytes(),
            mime: "text/html".to_string(),
            error_msg: msg.to_string(),
            ..Default::default()
        }
    }

    pub fn status(&self) -> HttpStatus {
        self.status
    }

    pub fn set_status(&mut self, status: HttpStatus) {
        self.status = status;
    }

    pub fn mime(&self) -> &str {
        &self.mime
    }

    pub fn error_page_required(&self) -> bool {
        self.status() != HttpStatus::Ok
    }

    pub fn error_msg(&self) -> &str {
        &self.error_msg
    }

    pub fn extra_html_headers(&self) -> &[String] {
        &self.extra_html_headers
    }

    pub fn set_status_as_body(&mut self) {
        self.body = format!(r#"<p style="font-size: large;">{}</p>"#, self.status).into_bytes();
        self.mime = "text/html".to_string();
    }

    pub fn remove_error_msg(&mut self) {
        self.error_msg.clear();
    }

    pub fn add_http_header(&mut self, http_header: &str) {
        self.extra_http_headers.push(http_header.to_string());
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

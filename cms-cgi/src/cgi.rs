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

use anyhow::{self as ah, format_err as err};
use cms_ident::Ident;
use cms_socket::{CmsSocketConn, MsgSerde as _};
use cms_socket_back::{Msg, SOCK_FILE};
use querystrong::QueryStrong;
use std::{collections::HashMap, env, ffi::OsString, path::Path};
use tokio::io::{self, AsyncReadExt as _, AsyncWriteExt as _, Stdout};

const MAX_POST_BODY_LEN: u32 = 1024 * 1024;

fn get_cgienv(name: &str) -> OsString {
    env::var_os(name).unwrap_or_default()
}

fn get_cgienv_str(name: &str) -> ah::Result<String> {
    if let Ok(s) = get_cgienv(name).into_string() {
        Ok(s)
    } else {
        Err(err!("Environment variable '{name}' is not valid UTF-8."))
    }
}

fn get_cgienv_u32(name: &str) -> ah::Result<u32> {
    Ok(get_cgienv_str(name)?.parse::<u32>()?)
}

fn get_cgienv_bool(name: &str) -> bool {
    get_cgienv(name).as_encoded_bytes() == b"on"
}

async fn out(f: &mut Stdout, data: &[u8]) {
    f.write_all(data).await.unwrap();
}

async fn outstr(f: &mut Stdout, data: &str) {
    out(f, data.as_bytes()).await;
}

async fn response_200_ok(body: Option<&[u8]>, mime: &str, extra_headers: &[String]) {
    let mut f = io::stdout();
    outstr(&mut f, &format!("Content-type: {mime}\n")).await;
    for header in extra_headers {
        outstr(&mut f, &format!("{header}\n")).await;
    }
    outstr(&mut f, "Status: 200 Ok\n").await;
    outstr(&mut f, "\n").await;
    if let Some(body) = body {
        out(&mut f, body).await;
    }
}

async fn response_400_bad_request(err: &str) {
    let mut f = io::stdout();
    outstr(&mut f, "Content-type: text/plain\n").await;
    outstr(&mut f, "Status: 400 Bad Request\n").await;
    outstr(&mut f, "\n").await;
    outstr(&mut f, err).await;
}

async fn response_500_internal_error(err: &str) {
    let mut f = io::stdout();
    outstr(&mut f, "Content-type: text/plain\n").await;
    outstr(&mut f, "Status: 500 Internal Server Error\n").await;
    outstr(&mut f, "\n").await;
    outstr(&mut f, err).await;
}

async fn response_notok(status: u32, body: Option<&[u8]>, mime: &str) {
    let mut f = io::stdout();
    outstr(&mut f, &format!("Content-type: {mime}\n")).await;
    outstr(&mut f, &format!("Status: {status}\n")).await;
    outstr(&mut f, "\n").await;
    if let Some(body) = body {
        out(&mut f, body).await;
    }
}

pub struct Cgi {
    query: String,
    meth: OsString,
    path: String,
    body_len: u32,
    body_type: String,
    https: bool,
    host: String,
    cookie: OsString,
    backend: CmsSocketConn,
}

impl Cgi {
    pub async fn new() -> ah::Result<Self> {
        let sock_path = Path::new("/run").join(SOCK_FILE);
        let Ok(backend) = CmsSocketConn::connect(&sock_path).await else {
            response_500_internal_error("Backend connection failed.").await;
            return Err(err!("Backend connection failed."));
        };

        //TODO: We can restrict syscalls with seccomp here.

        let query = get_cgienv_str("QUERY_STRING").unwrap_or_default();
        let meth = get_cgienv("REQUEST_METHOD");
        let path = get_cgienv_str("PATH_INFO").unwrap_or_default();
        let body_len = get_cgienv_u32("CONTENT_LENGTH").unwrap_or_default();
        let body_type = get_cgienv_str("CONTENT_TYPE").unwrap_or_default();
        let https = get_cgienv_bool("HTTPS");
        let host = get_cgienv_str("HTTP_HOST").unwrap_or_default();
        let cookie = get_cgienv("HTTP_COOKIE");

        Ok(Self {
            query,
            meth,
            path,
            body_len,
            body_type,
            https,
            host,
            cookie,
            backend,
        })
    }

    pub async fn run(&mut self) {
        let Ok(path) = self.path.parse::<Ident>() else {
            response_400_bad_request("Failed to parse PATH_INFO string.").await;
            return;
        };
        let Ok(path) = path.into_checked_sys() else {
            response_400_bad_request("URI path contains invalid chars.").await;
            return;
        };

        let Ok(q) = QueryStrong::parse(&self.query) else {
            response_400_bad_request("Invalid QUERY_STRING in URI.").await;
            return;
        };
        let mut query = HashMap::with_capacity(q.len());
        if let Some(q) = q.as_map() {
            for (n, v) in q {
                if let querystrong::Value::String(v) = v {
                    query.insert(n.to_string(), v.as_bytes().to_vec());
                }
            }
        }

        let meth = self.meth.as_encoded_bytes();
        match meth {
            b"GET" | b"HEAD" => {
                let request = Msg::Get {
                    host: self.host.clone(),
                    path: path.downgrade(),
                    https: self.https,
                    cookie: self.cookie.as_encoded_bytes().to_vec(),
                    query,
                };
                if self.backend.send_msg(&request).await.is_err() {
                    response_500_internal_error("Backend send failed.").await;
                    return;
                }
            }
            b"POST" => {
                if self.body_len == 0 {
                    response_400_bad_request("POST: CONTENT_LENGTH is zero.").await;
                    return;
                }
                if self.body_len > MAX_POST_BODY_LEN {
                    response_400_bad_request("POST: CONTENT_LENGTH is too large.").await;
                    return;
                }
                if self.body_type.is_empty() {
                    response_400_bad_request("POST: Invalid CONTENT_TYPE.").await;
                    return;
                }

                let mut body = vec![0; self.body_len.try_into().unwrap()];
                if io::stdin().read_exact(&mut body).await.is_err() {
                    response_500_internal_error("CGI stdin read failed.").await;
                    return;
                }

                let request = Msg::Post {
                    host: self.host.clone(),
                    path: path.downgrade(),
                    https: self.https,
                    cookie: self.cookie.as_encoded_bytes().to_vec(),
                    query,
                    body,
                    body_mime: self.body_type.clone(),
                };
                if self.backend.send_msg(&request).await.is_err() {
                    response_500_internal_error("Backend send failed.").await;
                    return;
                }
            }
            _ => {
                let meth = self.meth.to_string_lossy();
                response_400_bad_request(&format!("Unsupported REQUEST_METHOD: '{meth}'")).await;
                return;
            }
        }

        let msg = self.backend.recv_msg(Msg::try_msg_deserialize).await;
        match msg {
            Ok(Some(Msg::Reply {
                status,
                body,
                mime,
                extra_headers,
            })) => {
                let body = if meth == b"HEAD" {
                    None
                } else {
                    Some(&body[..])
                };
                match status {
                    200 => response_200_ok(body, &mime, &extra_headers).await,
                    status => response_notok(status, body, &mime).await,
                }
            }
            Ok(Some(Msg::Get { .. })) | Ok(Some(Msg::Post { .. })) => {
                response_500_internal_error("Invalid backend message received.").await;
            }
            Ok(None) => {
                response_500_internal_error("Backend disconnected.").await;
            }
            Err(_) => {
                response_500_internal_error("Backend receive error.").await;
            }
        }
    }
}

// vim: ts=4 sw=4 expandtab

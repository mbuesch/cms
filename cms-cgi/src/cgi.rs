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
use cms_ident::{CheckedIdent, Ident};
use cms_socket::CmsSocketConn;
use cms_socket_back::{Msg, SOCK_FILE};
use querystrong::QueryStrong;
use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    io::{self, Write as _},
    path::Path,
};

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

pub struct Cgi {
    query: HashMap<String, Vec<u8>>,
    meth: OsString,
    path: CheckedIdent,
    body_len: u32,
    body_type: OsString,
    https: bool,
    host: OsString,
    cookie: OsString,
    backend: CmsSocketConn,
}

impl Cgi {
    fn send_response_200_ok(body: &[u8], mime: &str) {
        let mut f = io::stdout();
        let _ = writeln!(f, "Content-type: {mime}");
        let _ = writeln!(f, "Status: 200 Ok");
        let _ = writeln!(f);
        let _ = f.write_all(body);
    }

    fn send_response_400_bad_request(err: ah::Error) -> ah::Error {
        let mut f = io::stdout();
        let _ = writeln!(f, "Content-type: text/plain");
        let _ = writeln!(f, "Status: 400 Bad Request");
        let _ = writeln!(f);
        let _ = writeln!(f, "{err}");
        err
    }

    fn send_response_404_not_found(err: ah::Error) -> ah::Error {
        let mut f = io::stdout();
        let _ = writeln!(f, "Location: /cms/__nopage/__nogroup.html");
        //let _ = writeln!(f, "Content-type: text/plain");
        //let _ = writeln!(f, "Status: 404 Not Found");
        let _ = writeln!(f);
        //let _ = writeln!(f, "{err}");
        err
    }

    fn send_response_500_internal_error(err: ah::Error) -> ah::Error {
        let mut f = io::stdout();
        let _ = writeln!(f, "Content-type: text/plain");
        let _ = writeln!(f, "Status: 500 Internal Server Error");
        let _ = writeln!(f);
        let _ = writeln!(f, "{err}");
        err
    }

    pub async fn new() -> ah::Result<Self> {
        let q = get_cgienv_str("QUERY_STRING").unwrap_or_default();
        let q = QueryStrong::parse(&q).map_err(|_| {
            Self::send_response_400_bad_request(err!("Invalid QUERY_STRING in URI."))
        })?;
        let mut query = HashMap::with_capacity(q.len());
        if let Some(q) = q.as_map() {
            for (n, v) in q {
                if let querystrong::Value::String(v) = v {
                    query.insert(n.to_string(), v.as_bytes().to_vec());
                }
            }
        }

        let meth = get_cgienv("REQUEST_METHOD");

        let path = get_cgienv_str("PATH_INFO").unwrap_or_default();
        let path = path.parse::<Ident>().map_err(|_| {
            Self::send_response_400_bad_request(err!("Failed to parse PATH_INFO string."))
        })?;
        let path = path.into_checked().map_err(|_| {
            Self::send_response_404_not_found(err!("URI path contains invalid characters."))
        })?;

        let body_len = get_cgienv_u32("CONTENT_LENGTH").unwrap_or_default();

        let body_type = get_cgienv("CONTENT_TYPE");

        let https = get_cgienv_bool("HTTPS");

        let host = get_cgienv("HTTP_HOST");

        let cookie = get_cgienv("HTTP_COOKIE");

        let sock_path = Path::new("/run").join(SOCK_FILE);
        let backend = CmsSocketConn::connect(&sock_path).await.map_err(|_| {
            Self::send_response_500_internal_error(err!("Backend connection failed."))
        })?;

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

    pub async fn run(&mut self) -> ah::Result<()> {
        match self.meth.as_encoded_bytes() {
            b"GET" => self.run_get().await,
            b"POST" => self.run_post().await,
            _ => Err(Self::send_response_400_bad_request(err!(
                "Unsupported REQUEST_METHOD: '{}'",
                self.meth.to_string_lossy()
            ))),
        }
    }

    async fn run_get(&mut self) -> ah::Result<()> {
        let request = Msg::Get {
            host: self.host.as_encoded_bytes().to_vec(),
            path: self.path.clone().downgrade(),
            https: self.https,
            cookie: self.cookie.as_encoded_bytes().to_vec(),
            query: self.query.clone(),
        };
        self.backend.send_msg(&request).await?;
        //TODO
        Ok(())
    }

    async fn run_post(&self) -> ah::Result<()> {
        if self.body_len == 0 {
            return Err(err!("POST: Invalid CONTENT_LENGTH."));
        }
        if self.body_type.is_empty() {
            return Err(err!("POST: Invalid CONTENT_TYPE."));
        }
        //TODO
        Ok(())
    }
}

// vim: ts=4 sw=4 expandtab

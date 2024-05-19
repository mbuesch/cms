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

use crate::{cookie::Cookie, query::Query};
use cms_ident::CheckedIdent;

pub fn html_safe_escape(text: &str) -> String {
    html_escape::encode_safe(text).to_string()
}

pub struct CmsGetArgs {
    pub host: String,
    pub path: CheckedIdent,
    pub _cookie: Cookie,
    pub query: Query,
    pub https: bool,
}

impl CmsGetArgs {
    pub fn protocol_str(&self) -> &str {
        if self.https {
            "https"
        } else {
            "http"
        }
    }
}

pub struct CmsPostArgs {
    pub body: Vec<u8>,
    pub body_mime: String,
}

pub fn get_query_var(get: &CmsGetArgs, variable_name: &str, escape: bool) -> String {
    if let Some(index) = variable_name.find('_') {
        let qname = &variable_name[index + 1..];
        if !qname.is_empty() {
            let qvalue = get.query.get_str(qname).unwrap_or_default();
            if escape {
                return html_safe_escape(&qvalue);
            } else {
                return qvalue;
            }
        }
    }
    Default::default()
}

// vim: ts=4 sw=4 expandtab

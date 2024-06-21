// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{cookie::Cookie, query::Query};
use cms_ident::CheckedIdent;

pub fn html_safe_escape(text: &str) -> String {
    html_escape::encode_safe(text).to_string()
}

pub struct CmsGetArgs {
    pub _host: String,
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

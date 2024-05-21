// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::numparse::parse_bool;
use anyhow::{self as ah, format_err as err};
use configparser::ini::Ini;

const CONF_PATH: &str = "/opt/cms/etc/cms/backd.conf";
const SECT: &str = "CMS-BACKD";

fn get_debug(ini: &Ini) -> ah::Result<bool> {
    if let Some(debug) = ini.get(SECT, "debug") {
        return parse_bool(&debug);
    }
    Ok(false)
}

fn get_domain(ini: &Ini) -> ah::Result<String> {
    if let Some(domain) = ini.get(SECT, "domain") {
        for c in domain.chars() {
            if !c.is_ascii_alphanumeric() && c != '.' && c != '-' {
                return Err(err!("'domain' has an invalid value."));
            }
        }
        return Ok(domain);
    }
    Ok("example.com".to_string())
}

fn get_url_base(ini: &Ini) -> ah::Result<String> {
    if let Some(url_base) = ini.get(SECT, "url-base") {
        for c in url_base.chars() {
            if !c.is_ascii_alphanumeric() && c != '/' && c != '_' && c != '-' {
                return Err(err!("'url-base' has an invalid value."));
            }
        }
        return Ok(url_base);
    }
    Ok("/cms".to_string())
}

pub struct CmsConfig {
    debug: bool,
    domain: String,
    url_base: String,
}

impl CmsConfig {
    pub fn new() -> ah::Result<Self> {
        let mut ini = Ini::new_cs();
        if let Err(e) = ini.load(CONF_PATH) {
            return Err(err!("Failed to load configuration {CONF_PATH}: {e}"));
        };

        let debug = get_debug(&ini)?;
        let domain = get_domain(&ini)?;
        let url_base = get_url_base(&ini)?;

        Ok(Self {
            debug,
            domain,
            url_base,
        })
    }

    pub fn debug(&self) -> bool {
        self.debug
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn url_base(&self) -> &str {
        &self.url_base
    }
}

// vim: ts=4 sw=4 expandtab

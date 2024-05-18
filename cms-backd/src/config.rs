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
use configparser::ini::Ini;

const CONF_PATH: &str = "/opt/cms/etc/cms/backd.conf";
const SECT: &str = "CMS-BACKD";

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
    domain: String,
    url_base: String,
}

impl CmsConfig {
    pub fn new() -> ah::Result<Self> {
        let mut ini = Ini::new_cs();
        if let Err(e) = ini.load(CONF_PATH) {
            return Err(err!("Failed to load configuration {CONF_PATH}: {e}"));
        };

        let domain = get_domain(&ini)?;
        let url_base = get_url_base(&ini)?;

        Ok(Self { domain, url_base })
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn url_base(&self) -> &str {
        &self.url_base
    }
}

// vim: ts=4 sw=4 expandtab

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

use crate::{comm::CmsComm, config::CmsConfig};
use anyhow as ah;
use cms_ident::CheckedIdent;
use std::sync::Arc;

pub struct SiteMapContext<'a> {
    pub comm: &'a mut CmsComm,
    pub config: Arc<CmsConfig>,
    pub root: &'a CheckedIdent,
    pub protocol: &'a str,
}

/// Site map generator.
/// Specification: https://www.sitemaps.org/protocol.html
pub struct SiteMap {}

impl SiteMap {
    pub async fn build(ctx: SiteMapContext<'_>) -> ah::Result<Self> {
        Ok(Self {})
    }

    pub fn get_xml(&self) -> String {
        String::new() //TODO
    }
}

// vim: ts=4 sw=4 expandtab

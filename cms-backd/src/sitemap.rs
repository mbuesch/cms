// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

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

// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    comm::{CmsComm, CommGetPage, CommPage, CommSubPages},
    config::CmsConfig,
};
use anyhow as ah;
use async_recursion::async_recursion;
use cms_ident::{CheckedIdent, UrlComp};
use std::{fmt::Write as _, sync::Arc, write as wr, writeln as ln};

const MAX_DEPTH: usize = 64;
const DEFAULT_ELEMS_ALLOC: usize = 256;
const DEFAULT_HTML_ALLOC: usize = 1024 * 16;

fn xml_escape(mut s: String) -> String {
    if !s.is_empty() {
        if s.contains('&') {
            s = s.replace('&', "&amp;");
        }
        if s.contains('\'') {
            s = s.replace('\'', "&apos;");
        }
        if s.contains('"') {
            s = s.replace('"', "&quot;");
        }
        if s.contains('>') {
            s = s.replace('>', "&gt;");
        }
        if s.contains('<') {
            s = s.replace('<', "&lt;");
        }
    }
    s
}

pub struct SiteMapContext<'a> {
    pub comm: &'a mut CmsComm,
    pub config: Arc<CmsConfig>,
    pub root: &'a CheckedIdent,
    pub protocol: &'a str,
}

struct SiteMapElem {
    depth: usize,
    loc: String,
    lastmod: String,
    changefreq: String,
    priority: String,
}

#[async_recursion]
async fn build_elems(
    ctx: &mut SiteMapContext<'_>,
    elems: &mut Vec<SiteMapElem>,
    ident: &CheckedIdent,
    depth: usize,
) -> ah::Result<()> {
    if depth >= MAX_DEPTH {
        return Ok(());
    }

    let Ok(CommPage {
        nav_stop, stamp, ..
    }) = ctx
        .comm
        .get_db_page(CommGetPage {
            path: ident.clone(),
            get_nav_stop: true,
            get_stamp: true,
            ..Default::default()
        })
        .await
    else {
        return Ok(());
    };
    if nav_stop.unwrap_or(true) {
        return Ok(());
    }
    let stamp = stamp.unwrap_or_default().format("%Y-%m-%dT%H:%M:%SZ");

    let Ok(CommSubPages { mut names, .. }) = ctx.comm.get_db_sub_pages(ident).await else {
        return Ok(());
    };

    let loc = ident.url(UrlComp {
        protocol: Some(ctx.protocol),
        domain: Some(ctx.config.domain()),
        base: Some(ctx.config.url_base()),
    });
    let lastmod;
    let changefreq;
    let priority;
    if depth == 1 {
        // Main groups
        lastmod = String::new();
        changefreq = "monthly".to_string();
        priority = "0.3".to_string();
    } else {
        // Pages, main page and sub groups
        lastmod = stamp.to_string();
        changefreq = String::new();
        priority = "0.7".to_string();
    }

    elems.push(SiteMapElem {
        depth,
        loc,
        lastmod,
        changefreq,
        priority,
    });

    names.sort_unstable();
    for name in &names {
        let sub_ident = ident.clone_append(name).into_checked()?;
        build_elems(ctx, elems, &sub_ident, depth + 1).await?;
    }

    Ok(())
}

/// Site map generator.
/// Specification: https://www.sitemaps.org/protocol.html
pub struct SiteMap {
    elems: Vec<SiteMapElem>,
}

impl SiteMap {
    pub async fn build(mut ctx: SiteMapContext<'_>) -> ah::Result<Self> {
        let mut elems = Vec::with_capacity(DEFAULT_ELEMS_ALLOC);
        let root = ctx.root.clone();
        build_elems(&mut ctx, &mut elems, &root, 0).await?;
        Ok(Self { elems })
    }

    fn add_user_url_elems(&self, b: &mut String) -> ah::Result<()> {
        //TODO
        Ok(())
    }

    fn add_url_elems(&self, b: &mut String) -> ah::Result<()> {
        for elem in &self.elems {
            let loc = xml_escape(elem.loc.clone());
            let lastmod = xml_escape(elem.lastmod.clone());
            let changefreq = xml_escape(elem.changefreq.clone());
            let priority = xml_escape(elem.priority.clone());

            ln!(b, r#"<url>"#)?;
            if !loc.is_empty() {
                ln!(b, r#"<loc>{loc}</loc>"#)?;
            }
            if !lastmod.is_empty() {
                ln!(b, r#"<lastmod>{lastmod}</lastmod>"#)?;
            }
            if !changefreq.is_empty() {
                ln!(b, r#"<changefreq>{changefreq}</changefreq>"#)?;
            }
            if !priority.is_empty() {
                ln!(b, r#"<priority>{priority}</priority>"#)?;
            }
            ln!(b, r#"</url>"#)?;
        }
        Ok(())
    }

    #[rustfmt::skip]
    pub fn get_xml(&self) -> ah::Result<String> {
        let mut b = String::with_capacity(DEFAULT_HTML_ALLOC);
        ln!(b, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        wr!(b, r#"<urlset xmlns="https://www.sitemaps.org/schemas/sitemap/0.9" "#)?;
        wr!(b, r#"xmlns:xsi="https://www.w3.org/2001/XMLSchema-instance" "#)?;
        wr!(b, r#"xsi:schemaLocation="https://www.sitemaps.org/schemas/sitemap/0.9 "#)?;
        ln!(b, r#"https://www.sitemaps.org/schemas/sitemap/0.9/sitemap.xsd">"#)?;
        self.add_url_elems(&mut b)?;
        self.add_user_url_elems(&mut b)?;
        wr!(b, r#"</urlset>"#)?;
        Ok(b)
    }
}

// vim: ts=4 sw=4 expandtab

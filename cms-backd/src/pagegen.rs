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

use crate::{
    backend::{CmsGetArgs, CmsReply},
    config::CmsConfig,
    navtree::{NavElem, NavTree},
};
use anyhow as ah;
use chrono::prelude::*;
use cms_ident::{CheckedIdent, UrlComp};
use std::{fmt::Write as _, sync::Arc, writeln as ln};

const DEFAULT_HTML_ALLOC: usize = 1024 * 64;

#[inline]
fn make_indent(indent: usize) -> &'static str {
    const TEMPLATE: &str = "                                        ";
    &TEMPLATE[..(indent * 4).min(TEMPLATE.len())]
}

pub struct PageGen<'a> {
    get: &'a CmsGetArgs,
    config: Arc<CmsConfig>,
}

impl<'a> PageGen<'a> {
    pub fn new(get: &'a CmsGetArgs, config: Arc<CmsConfig>) -> Self {
        Self { get, config }
    }

    #[rustfmt::skip]
    #[allow(clippy::only_used_in_recursion)]
    pub fn generate_navelem(
        &self,
        b: &mut String,
        navelems: &[NavElem],
        indent: usize,
    ) -> ah::Result<()> {
        if navelems.is_empty() {
            return Ok(());
        }

        let c = &self.config;
        let ii = make_indent(indent + 1);

        if indent > 0 {
            ln!(b, r#"{ii}<div class="navelems">"#)?;
        }

        for navelem in navelems {
            let nav_label = navelem.nav_label().trim();
            if nav_label.is_empty() {
                continue;
            }
            let nav_href = navelem.path().url(UrlComp {
                protocol: None,
                domain: None,
                base: Some(c.url_base()),
            });
            let prio = navelem.prio();

            let cls = if indent > 0 { "navelem" } else { "navgroup" };
            ln!(b, r#"{ii}    <div class="{cls}"> <!-- {prio} -->"#)?;

            if indent == 0 {
                ln!(b, r#"{ii}        <div class="navhead">"#)?;
            }

            if navelem.active() {
                ln!(b, r#"{ii}        <div class="navactive">"#)?;
            }

            ln!(b, r#"{ii}        <a href="{nav_href}">{nav_label}</a>"#)?;

            if navelem.active() {
                ln!(b, r#"{ii}        </div>"#)?; // navactive
            }

            if indent == 0 {
                ln!(b, r#"{ii}        </div>"#)?; // navhead
            }

            self.generate_navelem(b, navelem.children(), indent + 2)?;

            ln!(b, r#"{ii}    </div>"#)?; // navelem / navgroup
        }

        if indent > 0 {
            ln!(b, r#"{ii}</div>"#)?; // navelems
        }
        Ok(())
    }

    #[rustfmt::skip]
    fn generate_nav(
        &self,
        b: &mut String,
        navtree: &NavTree,
        homestr: &str,
    ) -> ah::Result<()> {
        let c = &self.config;
        let nav_home_href = CheckedIdent::ROOT.url(UrlComp {
            protocol: None,
            domain: None,
            base: Some(c.url_base()),
        });
        let nav_home_text = homestr.trim();

        ln!(b, r#"<div class="navbar">"#)?;
        ln!(b, r#"    <div class="navgroups">"#)?;
        ln!(b, r#"        <div class="navhome">"#)?;
        if self.get.path.is_root() {
            ln!(b, r#"        <div class="navactive">"#)?;
        }
        ln!(b, r#"            <a href="{nav_home_href}">{nav_home_text}</a>"#)?;
        if self.get.path.is_root() {
            ln!(b, r#"        </div>"#)?; // navactive
        }
        ln!(b, r#"        </div>"#)?; // navhome

        self.generate_navelem(b, navtree.elems(), 0)?;

        ln!(b, r#"    </div>"#)?; // navgroups
        ln!(b, r#"</div>"#)?; // navbar

        Ok(())
    }

    #[rustfmt::skip]
    fn generate_body(
        &self,
        b: &mut String,
        title: &str,
        page_content: &str,
        stamp: &DateTime<Utc>,
        navtree: &NavTree,
        homestr: &str,
    ) -> ah::Result<()> {
        let c = &self.config;
        let page_stamp = stamp.format("%A %d %B %Y %H:%M");
        let page_checker = ""; //TODO

        ln!(b, r#"<div class="titlebar">"#)?;
        ln!(b, r#"    <div class="logo">"#)?;
        ln!(b, r#"        <a href="{}">"#, c.url_base())?;
        ln!(b, r#"            <img alt="logo" src="{}/__images/logo.png" />"#, c.url_base())?;
        ln!(b, r#"        </a>"#)?;
        ln!(b, r#"    </div>"#)?;
        ln!(b, r#"    <div class="title">{title}</div>"#)?;
        ln!(b, r#"</div>"#)?;
        self.generate_nav(b, navtree, homestr)?;
        ln!(b, r#"<div class="main">"#)?;
        ln!(b)?;
        ln!(b, r#"<!-- BEGIN: page content -->"#)?;
        ln!(b, r#"{page_content}"#)?;
        ln!(b, r#"<!-- END: page content -->"#)?;
        ln!(b)?;
        ln!(b, r#"<div class="modifystamp">"#)?;
        ln!(b, r#"Updated: {page_stamp} (UTC)"#)?;
        ln!(b, r#"</div>"#)?;
        ln!(b)?;
        ln!(b, r#"{page_checker}"#)?;
        ln!(b)?;
        ln!(b, r#"</div> <!-- class="main" -->"#)?;
        Ok(())
    }

    #[rustfmt::skip]
    #[allow(clippy::too_many_arguments)]
    pub fn generate_html(
        &self,
        title: &str,
        headers: &str,
        data: &str,
        now: &DateTime<Utc>,
        stamp: &DateTime<Utc>,
        navtree: &NavTree,
        homestr: &str,
    ) -> ah::Result<String> {
        let c = &self.config;
        let mut b = String::with_capacity(DEFAULT_HTML_ALLOC);

        let title = title.trim();
        let now = now.to_rfc3339_opts(SecondsFormat::Secs, true);
        let extra_head = ""; //TODO

        ln!(b, r#"<?xml version="1.0" encoding="UTF-8" ?>"#)?;
        ln!(b, r#"<!DOCTYPE html>"#)?;
        ln!(b, r#"<html xmlns="http://www.w3.org/1999/xhtml" lang="en" xml:lang="en">"#)?;
        ln!(b, r#"<head>"#)?;
        ln!(b, r#"    <!--"#)?;
        ln!(b, r#"        Generated by: Simple Rust based CMS"#)?;
        ln!(b, r#"        https://bues.ch/cgit/cms.git/about/"#)?;
        ln!(b, r#"        https://github.com/mbuesch/cms"#)?;
        ln!(b, r#"    -->"#)?;
        ln!(b, r#"    <meta name="generator" content="Simple Rust based CMS" />"#)?;
        ln!(b, r#"    <meta name="date" content="{now}" />"#)?;
        ln!(b, r#"    <meta name="robots" content="all" />"#)?;
        ln!(b, r#"    <title>{title}</title>"#)?;
        ln!(b, r#"    <link rel="stylesheet" href="{}/__css/cms.css" type="text/css" />"#,
            c.url_base())?;
        ln!(b, r#"    <link rel="sitemap" type="application/xml" title="Sitemap" href="{}/__sitemap.xml" />"#,
            c.url_base())?;
        ln!(b, r#"    {extra_head}"#)?;
        ln!(b, r#"</head>"#)?;
        ln!(b, r#"<body>"#)?;
        self.generate_body(&mut b, title, data, stamp, navtree, homestr)?;
        ln!(b, r#"</body>"#)?;
        ln!(b, r#"</html>"#)?;
        Ok(b)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        &self,
        title: &str,
        headers: &str,
        data: &str,
        now: &DateTime<Utc>,
        stamp: &DateTime<Utc>,
        navtree: &NavTree,
        homestr: &str,
    ) -> CmsReply {
        if let Ok(b) = self.generate_html(title, headers, data, now, stamp, navtree, homestr) {
            CmsReply::ok(
                b.into_bytes(),
                "application/xhtml+xml; charset=UTF-8".to_string(),
            )
        } else {
            CmsReply::internal_error("PageGen failed")
        }
    }
}

// vim: ts=4 sw=4 expandtab

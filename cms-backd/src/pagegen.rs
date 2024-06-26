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
    anchor::Anchor,
    args::CmsGetArgs,
    config::CmsConfig,
    navtree::{NavElem, NavTree},
    reply::CmsReply,
    resolver::Resolver,
};
use anyhow::{self as ah, format_err as err};
use chrono::prelude::*;
use cms_ident::{CheckedIdent, UrlComp};
use std::{fmt::Write as _, sync::Arc, write as wr, writeln as ln};

const DEFAULT_HTML_ALLOC: usize = 1024 * 64;
const DEFAULT_INDEX_HTML_ALLOC: usize = 1024 * 4;
const MAX_INDENT: usize = 1024;

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

    #[allow(clippy::comparison_chain)]
    pub fn generate_index(&self, anchors: &[Anchor], resolver: &Resolver) -> ah::Result<String> {
        let mut html = String::with_capacity(DEFAULT_INDEX_HTML_ALLOC);

        ln!(html, r#"{}<ul>"#, make_indent(1))?;
        let mut indent = 0;

        for anchor in anchors {
            if anchor.no_index() || anchor.text().is_empty() {
                continue;
            }
            if let Some(aindent) = anchor.indent() {
                // Adjust indent.
                if aindent > indent {
                    if aindent > MAX_INDENT {
                        return Err(err!("Anchor indent too big"));
                    }
                    for _ in 0..(aindent - indent) {
                        indent += 1;
                        ln!(html, r#"{}<ul>"#, make_indent(indent + 1))?;
                    }
                } else if aindent < indent {
                    for _ in 0..(indent - aindent) {
                        ln!(html, r#"{}</ul>"#, make_indent(indent + 1))?;
                        indent -= 1;
                    }
                }
            }
            // Anchor data.
            ln!(
                html,
                r#"{}<li>{}</li>"#,
                make_indent(indent + 2),
                anchor.make_html(resolver, false)?,
            )?;
        }
        for _ in 0..(indent + 1) {
            ln!(html, r#"{}</ul>"#, make_indent(indent + 1))?;
        }
        Ok(html)
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
    #[allow(clippy::too_many_arguments)]
    fn generate_body(
        &self,
        b: &mut String,
        path: Option<&CheckedIdent>,
        title: &str,
        page_content: &str,
        stamp: &DateTime<Utc>,
        navtree: &NavTree,
        homestr: &str,
    ) -> ah::Result<()> {
        let c = &self.config;
        let page_stamp = stamp.format("%A %d %B %Y %H:%M");

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
        ln!(b, r#"    Updated: {page_stamp} (UTC)"#)?;
        ln!(b, r#"</div>"#)?;
        ln!(b)?;
        if let Some(path) = path {
            let url = path.url(UrlComp {
                protocol: Some("https"),
                domain: Some(self.config.domain()),
                base: Some(self.config.url_base()),
            });
            let mut url_enc = String::with_capacity(url.len() * 4);
            let url = url_escape::encode_component_to_string(url, &mut url_enc);

            ln!(b, r#"<div class="checker">"#)?;
            wr!(b, r#"    <a href="https://validator.w3.org/nu/"#)?;
            ln!(b, r#"?showsource=yes&amp;doc={url}">xhtml</a>"#)?;
            ln!(b, r#"    /"#)?;
            wr!(b, r#"    <a href="https://jigsaw.w3.org/css-validator/validator"#)?;
            wr!(b, r#"?uri={url}&amp;profile=css3svg&amp;usermedium=all&amp;warning=1"#)?;
            ln!(b, r#"&amp;vextwarning=&amp;lang=en">css</a>"#)?;
            ln!(b, r#"</div>"#)?;
        }
        ln!(b)?;
        ln!(b, r#"</div> <!-- class="main" -->"#)?;
        Ok(())
    }

    #[rustfmt::skip]
    #[allow(clippy::too_many_arguments)]
    pub fn generate_html(
        &self,
        path: Option<&CheckedIdent>,
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

        let headers = headers
            .lines()
            .fold(
                String::with_capacity(headers.len() * 2),
                |mut buf, line| {
                    let _ = ln!(buf, r#"    {line}"#);
                    buf
                }
            );

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
        ln!(b, r#"    <!-- extra headers: -->"#)?;
        ln!(b, r#"{headers}"#)?;
        ln!(b, r#"</head>"#)?;
        ln!(b, r#"<body>"#)?;
        self.generate_body(&mut b, path, title, data, stamp, navtree, homestr)?;
        ln!(b, r#"</body>"#)?;
        ln!(b, r#"</html>"#)?;
        Ok(b)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        &self,
        path: Option<&CheckedIdent>,
        title: &str,
        headers: &str,
        data: &str,
        now: &DateTime<Utc>,
        stamp: &DateTime<Utc>,
        navtree: &NavTree,
        homestr: &str,
    ) -> CmsReply {
        if let Ok(b) = self.generate_html(path, title, headers, data, now, stamp, navtree, homestr)
        {
            CmsReply::ok(b.into_bytes(), "application/xhtml+xml; charset=UTF-8")
        } else {
            CmsReply::internal_error("PageGen failed")
        }
    }
}

// vim: ts=4 sw=4 expandtab

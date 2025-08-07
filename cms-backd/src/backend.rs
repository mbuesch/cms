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
    args::{CmsGetArgs, CmsPostArgs, get_query_var, html_safe_escape},
    cache::CmsCache,
    comm::{CmsComm, CommGetPage, CommPage, CommPostHandlerResult, CommRunPostHandler},
    config::CmsConfig,
    formfields::FormFields,
    navtree::NavTree,
    pagegen::PageGen,
    reply::{CmsReply, HttpStatus},
    resolver::{Resolver, ResolverVars, getvar},
    sitemap::{SiteMap, SiteMapContext},
};
use anyhow as ah;
use chrono::prelude::*;
use cms_ident::{CheckedIdent, UrlComp};
use std::{io::Cursor, path::Path, sync::Arc};

#[rustfmt::skip]
macro_rules! make_resolver_vars {
    ($get:expr, $config:expr) => {{
        let mut vars = ResolverVars::new();
        vars.register(
            "PAGEIDENT",
            getvar!($get.path.url(UrlComp {
                protocol: None,
                domain: None,
                base: None,
            })),
        );
        vars.register(
            "CMS_PAGEIDENT",
            getvar!($get.path.url(UrlComp {
                protocol: None,
                domain: None,
                base: Some($config.url_base()),
            })),
        );
        vars.register("PROTOCOL", getvar!($get.protocol_str().to_string()));
        vars.register("GROUP", getvar!($get.path.nth_element_str(0).unwrap_or("").to_string()));
        vars.register("PAGE", getvar!($get.path.nth_element_str(1).unwrap_or("").to_string()));
        vars.register("DOMAIN", getvar!($config.domain().to_string()));
        vars.register("CMS_BASE", getvar!($config.url_base().to_string()));
        vars.register("IMAGES_DIR", getvar!(format!("{}/__images", $config.url_base())));
        vars.register("THUMBS_DIR", getvar!(format!("{}/__thumbs", $config.url_base())));
        vars.register("DEBUG", getvar!(if $config.debug() { "1" } else { "" }.to_string()));

        vars.register_prefix("Q", Arc::new(|name| get_query_var($get, name, true)));
        vars.register_prefix("QRAW", Arc::new(|name| get_query_var($get, name, false)));

        vars
    }};
}

macro_rules! resolve {
    ($comm:expr, $get:expr, $config:expr, $vars:expr, $text:expr) => {
        Resolver::new(&mut $comm, $get, Arc::clone(&$config), &$get.path, &$vars)
            .run(&$text)
            .await
    };
}

pub struct CmsBack {
    config: Arc<CmsConfig>,
    #[allow(dead_code)] //TODO
    cache: Arc<CmsCache>,
    comm: CmsComm,
}

impl CmsBack {
    pub async fn new(config: Arc<CmsConfig>, cache: Arc<CmsCache>, rundir: &Path) -> Self {
        Self {
            config,
            cache,
            comm: CmsComm::new(rundir),
        }
    }

    async fn get_page(&mut self, get: &CmsGetArgs) -> ah::Result<CmsReply> {
        // Get the page data.
        let Ok(CommPage {
            title,
            data,
            stamp,
            redirect,
            ..
        }) = self
            .comm
            .get_db_page(CommGetPage {
                path: get.path.clone(),
                get_title: true,
                get_data: true,
                get_stamp: true,
                get_redirect: true,
            })
            .await
        else {
            return Ok(CmsReply::internal_error("Invalid database reply"));
        };
        let mut title = title.unwrap_or_default();
        let mut data = data.unwrap_or_default();
        let stamp = stamp.unwrap_or_default();
        let redirect = redirect.unwrap_or_default();

        // Redirect to another page?
        if !redirect.is_empty() {
            return Ok(CmsReply::redirect(&redirect));
        }

        // Page not found?
        if data.is_empty() {
            let url = get.path.url(UrlComp {
                protocol: None,
                domain: Some(self.config.domain()),
                base: Some(self.config.url_base()),
            });
            return Ok(CmsReply::not_found(&format!("{url}: Page not available")));
        }

        // Get the page header data and strings.
        let mut headers = self
            .comm
            .get_db_headers(&get.path)
            .await
            .unwrap_or_default();
        let mut homestr = self.comm.get_db_string("home").await.unwrap_or_default();

        // Build the navigation tree.
        let navtree = NavTree::build(&mut self.comm, &CheckedIdent::ROOT, Some(&get.path)).await;

        // Resolve all data and strings.
        let mut vars = make_resolver_vars!(get, self.config);
        title = resolve!(self.comm, get, self.config, vars, title)?;
        vars.register("TITLE", getvar!(title.clone()));
        data = resolve!(self.comm, get, self.config, vars, data)?;
        headers = resolve!(self.comm, get, self.config, vars, headers)?;
        homestr = resolve!(self.comm, get, self.config, vars, homestr)?;

        // Generate the page.
        let now = Utc::now();
        Ok(PageGen::new(get, Arc::clone(&self.config)).generate(
            Some(&get.path),
            &title,
            &headers,
            &data,
            &now,
            &stamp,
            &navtree,
            &homestr,
        ))
    }

    async fn get_image(&mut self, get: &CmsGetArgs, thumb: bool) -> ah::Result<CmsReply> {
        let Some(img_name) = get.path.nth_element(1) else {
            return Ok(CmsReply::not_found("Invalid image path"));
        };
        let Ok(img_name) = img_name.into_checked_element() else {
            return Ok(CmsReply::not_found("Invalid image path"));
        };
        let img_data = match self.comm.get_db_image(&img_name).await {
            Ok(img_data) => img_data,
            Err(_) => return Ok(CmsReply::not_found("Image not found")),
        };
        if img_name.ends_with(".svg") {
            Ok(CmsReply::ok(img_data, "image/svg+xml"))
        } else {
            let img_cursor = Cursor::new(&img_data);
            let image = match image::ImageReader::new(img_cursor).with_guessed_format() {
                Ok(image) => image,
                Err(_) => return Ok(CmsReply::not_found("Invalid image format")),
            };
            let mime = match image.format() {
                Some(image::ImageFormat::Png) => "image/png",
                Some(image::ImageFormat::Gif) => "image/gif",
                Some(image::ImageFormat::WebP) => "image/webp",
                Some(image::ImageFormat::Jpeg) => "image/jpeg",
                _ => return Ok(CmsReply::not_found("Unsupported image format")),
            };
            if thumb {
                let image = match image.decode() {
                    Ok(image) => image,
                    Err(_) => return Ok(CmsReply::not_found("Image decode failed")),
                };
                let width = get.query.get_int("w").unwrap_or(300);
                let height = get.query.get_int("h").unwrap_or(300);
                let quality = match get.query.get_int("q").unwrap_or(1).clamp(0, 3) {
                    0 => 65,
                    1 => 75,
                    2 => 85,
                    _ => 95,
                };
                let image = image.thumbnail(
                    width.clamp(0, 1024 * 64).try_into().unwrap(),
                    height.clamp(0, 1024 * 64).try_into().unwrap(),
                );
                let mut img_data = Vec::with_capacity(img_data.len());
                let mut enc =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut img_data, quality);
                if enc.encode_image(&image).is_err() {
                    return Ok(CmsReply::internal_error("Thumbnail encoding failed"));
                };
                Ok(CmsReply::ok(img_data, "image/jpeg"))
            } else {
                Ok(CmsReply::ok(img_data, mime))
            }
        }
    }

    async fn get_sitemap(&mut self, get: &CmsGetArgs) -> ah::Result<CmsReply> {
        let sitemap = SiteMap::build(SiteMapContext {
            comm: &mut self.comm,
            config: Arc::clone(&self.config),
            root: &CheckedIdent::ROOT,
            protocol: get.protocol_str(),
        })
        .await?;
        let xml = sitemap.get_xml()?;
        Ok(CmsReply::ok(xml.into_bytes(), "text/xml; charset=UTF-8"))
    }

    async fn get_css(&mut self, get: &CmsGetArgs) -> ah::Result<CmsReply> {
        if let Some(css_name) = get.path.nth_element_str(1)
            && css_name == "cms.css"
        {
            let css = self.comm.get_db_string("css").await;
            return Ok(match css {
                Ok(body) => CmsReply::ok(body.into_bytes(), "text/css; charset=UTF-8"),
                Err(e) => CmsReply::not_found(&e.to_string()),
            });
        }
        Ok(CmsReply::not_found("Invalid CSS name"))
    }

    #[rustfmt::skip]
    pub async fn get(&mut self, get: &CmsGetArgs) -> CmsReply {
        let count = get.path.element_count();
        let first = get.path.first_element_str();

        let mut reply: CmsReply = match first {
            Some("__thumbs") if count == 2 => {
                self.get_image(get, true).await.into()
            }
            Some("__images") if count == 2 => {
                self.get_image(get, false).await.into()
            }
            Some("__sitemap") | Some("__sitemap.xml") if count == 1 => {
                self.get_sitemap(get).await.into()
            }
            Some("__css") if count == 2 => {
                self.get_css(get).await.into()
            }
            _ => {
                self.get_page(get).await.into()
            }
        };

        if reply.error_page_required() {
            // Generate a human readable error page.
            reply = self.get_error_page(get, reply).await;
        } else {
            // Add Cache-Control header.
            let mime = reply.mime();
            if mime.contains("html") {
                reply.add_http_header("Cache-Control: max-age=10");
            } else if mime.starts_with("image/") {
                reply.add_http_header("Cache-Control: max-age=3600");
            } else if mime.starts_with("text/css") {
                reply.add_http_header("Cache-Control: max-age=600");
            }
        }

        reply
    }

    async fn do_post(&mut self, get: &CmsGetArgs, post: &CmsPostArgs) -> ah::Result<CmsReply> {
        let path = get.path.clone_append("post.py").into_checked()?;
        let form_fields = match FormFields::new(&post.body, &post.body_mime).await {
            Ok(form_fields) => form_fields,
            Err(e) => {
                return Ok(CmsReply::bad_request(&format!(
                    "Failed to parse form-fields: {e}"
                )));
            }
        };

        let Ok(CommPostHandlerResult { error, body, mime }) = self
            .comm
            .run_post_handler(CommRunPostHandler {
                path,
                query: get.query.clone().into_items(),
                form_fields: form_fields.into_items(),
            })
            .await
        else {
            return Ok(CmsReply::internal_error("Invalid postd reply"));
        };
        let error = error.trim();

        if !error.is_empty() {
            return Ok(CmsReply::bad_request(&format!(
                "Failed to run POST handler: {error}"
            )));
        }

        Ok(CmsReply::ok(body, &mime))
    }

    pub async fn post(&mut self, get: &CmsGetArgs, post: &CmsPostArgs) -> CmsReply {
        let mut reply: CmsReply = self.do_post(get, post).await.into();

        if reply.error_page_required() {
            // Generate a human readable error page.
            reply = self.get_error_page(get, reply).await;
        } else {
            // Add Cache-Control header.
            reply.add_http_header("Cache-Control: no-cache");
        }

        reply
    }

    async fn get_error_page(&mut self, get: &CmsGetArgs, mut error: CmsReply) -> CmsReply {
        let orig_status = error.status();

        // Remove detailed error information, if not debugging.
        if error.status() == HttpStatus::InternalServerError && !self.config.debug() {
            error.set_status_as_body();
            error.remove_error_msg();
        }

        // Get the error page HTML code.
        let error_page_html = self.comm.get_db_string("http-error-page").await;
        let mut error_page_html = error_page_html.unwrap_or_default();
        if error_page_html.trim().is_empty() {
            error_page_html = format!(r#"<p style="font-size: large;">{}</p>"#, error.status());
        }

        // Prepare the resolver.
        let http_status_str = error.status().to_string();
        let http_status_code_str = (error.status() as u16).to_string();
        let mut error_msg = error.error_msg().to_string();
        if error_msg.is_empty() {
            error_msg.clone_from(&http_status_code_str);
        }
        error_msg = html_safe_escape(&error_msg);
        let title = error.status().to_string();
        let mut vars = make_resolver_vars!(get, self.config);
        vars.register("GROUP", getvar!("_error_".to_string()));
        vars.register("PAGE", getvar!("_error_".to_string()));
        vars.register("TITLE", getvar!(title.clone()));
        vars.register("HTTP_STATUS", getvar!(http_status_str.clone()));
        vars.register("HTTP_STATUS_CODE", getvar!(http_status_code_str.clone()));
        vars.register("ERROR_MESSAGE", getvar!(error_msg.clone()));

        // Get html headers.
        let html_headers = error.extra_html_headers().join("\n");
        let html_headers =
            resolve!(self.comm, get, self.config, vars, html_headers).unwrap_or_default();

        // Resolve the body.
        let Ok(error_page_html) = resolve!(self.comm, get, self.config, vars, error_page_html)
        else {
            return error;
        };

        // Generate the page.
        let homestr = self.comm.get_db_string("home").await.unwrap_or_default();
        let homestr = resolve!(self.comm, get, self.config, vars, homestr).unwrap_or_default();
        let navtree = NavTree::build(&mut self.comm, &CheckedIdent::ROOT, Some(&get.path)).await;
        let now = Utc::now();
        error = PageGen::new(get, Arc::clone(&self.config)).generate(
            None,
            &title,
            &html_headers,
            &error_page_html,
            &now,
            &now,
            &navtree,
            &homestr,
        );

        error.set_status(orig_status);
        error.add_http_header("Cache-Control: no-store");
        error
    }
}

// vim: ts=4 sw=4 expandtab

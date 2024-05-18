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
    args::{get_query_var, CmsGetArgs, CmsPostArgs},
    cache::CmsCache,
    comm::CmsComm,
    config::CmsConfig,
    navtree::NavTree,
    pagegen::PageGen,
    reply::{CmsReply, HttpStatus},
    resolver::{getvar, Resolver, ResolverVars},
};
use anyhow as ah;
use chrono::prelude::*;
use cms_ident::{CheckedIdent, UrlComp};
use cms_socket_db::Msg as MsgDb;
use cms_socket_post::Msg as MsgPost;
use std::{io::Cursor, path::Path, sync::Arc};

fn epoch_stamp(seconds: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds.try_into().unwrap_or_default(), 0).unwrap_or_default()
}

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
        vars.register(
            "GROUP",
            getvar!($get.path.nth_element_str(0).unwrap_or("").to_string()),
        );
        vars.register(
            "PAGE",
            getvar!($get.path.nth_element_str(1).unwrap_or("").to_string()),
        );
        vars.register("DOMAIN", getvar!($config.domain().to_string()));
        vars.register("CMS_BASE", getvar!($config.url_base().to_string()));
        vars.register(
            "IMAGES_DIR",
            getvar!(format!("{}/__images", $config.url_base())),
        );
        vars.register(
            "THUMBS_DIR",
            getvar!(format!("{}/__thumbs", $config.url_base())),
        );
        vars.register(
            "DEBUG",
            getvar!(if $config.debug() { "1" } else { "" }.to_string()),
        );

        vars.register_prefix("Q", Arc::new(|name| get_query_var($get, name, true)));
        vars.register_prefix("QRAW", Arc::new(|name| get_query_var($get, name, false)));

        vars
    }};
}

macro_rules! resolve {
    ($comm:expr, $get:expr, $config:expr, $vars:expr, $text:expr) => {
        Resolver::new(&mut $comm, $get, Arc::clone(&$config), &$get.path, &$vars)
            .run(&$text)
            .await?
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
        let reply = self
            .comm
            .comm_db(&MsgDb::GetPage {
                path: get.path.downgrade_clone(),
                get_title: true,
                get_data: true,
                get_stamp: true,
                get_prio: false,
                get_redirect: true,
                get_nav_stop: false,
                get_nav_label: false,
            })
            .await;
        let Ok(MsgDb::Page {
            title,
            data,
            stamp,
            redirect,
            ..
        }) = reply
        else {
            return Ok(CmsReply::internal_error("GetPage: Invalid db reply"));
        };

        let redirect = String::from_utf8(redirect.unwrap_or_default()).unwrap_or_default();
        if !redirect.is_empty() {
            return Ok(CmsReply::redirect(&redirect));
        }

        if data.is_none() {
            return Ok(CmsReply::not_found("GetPage: Page not available"));
        }

        let mut title = String::from_utf8(title.unwrap_or_default()).unwrap_or_default();
        let mut data = String::from_utf8(data.unwrap_or_default()).unwrap_or_default();
        let stamp = epoch_stamp(stamp.unwrap_or_default());

        let reply = self
            .comm
            .comm_db(&MsgDb::GetHeaders {
                path: get.path.downgrade_clone(),
            })
            .await;
        let Ok(MsgDb::Headers { data: headers }) = reply else {
            return Ok(CmsReply::internal_error("GetHeaders: Invalid db reply"));
        };

        let mut headers = String::from_utf8(headers).unwrap_or_default();
        let navtree = NavTree::build(&mut self.comm, &CheckedIdent::ROOT, Some(&get.path)).await;
        let mut homestr = self.comm.get_db_string("home").await.unwrap_or_default();

        let mut vars = make_resolver_vars!(get, self.config);
        title = resolve!(self.comm, get, self.config, vars, title);
        vars.register("TITLE", getvar!(title.clone()));
        data = resolve!(self.comm, get, self.config, vars, data);
        headers = resolve!(self.comm, get, self.config, vars, headers);
        homestr = resolve!(self.comm, get, self.config, vars, homestr);

        let now = Utc::now();
        Ok(PageGen::new(get, Arc::clone(&self.config))
            .generate(&title, &headers, &data, &now, &stamp, &navtree, &homestr))
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
            let image = match image::io::Reader::new(img_cursor).with_guessed_format() {
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
        //TODO
        Ok(Default::default())
    }

    async fn get_css(&mut self, get: &CmsGetArgs) -> ah::Result<CmsReply> {
        if let Some(css_name) = get.path.nth_element_str(1) {
            if css_name == "cms.css" {
                let css = self.comm.get_db_string("css").await;
                return Ok(match css {
                    Ok(body) => CmsReply::ok(body.into_bytes(), "text/css; charset=UTF-8"),
                    Err(e) => CmsReply::not_found(&e.to_string()),
                });
            }
        }
        Ok(CmsReply::not_found("Invalid CSS name"))
    }

    pub async fn get(&mut self, get: &CmsGetArgs) -> CmsReply {
        let count = get.path.element_count();
        let first = get.path.first_element_str();
        let reply: CmsReply = match first {
            Some("__thumbs") if count == 2 => self.get_image(get, true).await.into(),
            Some("__images") if count == 2 => self.get_image(get, false).await.into(),
            Some("__sitemap") | Some("__sitemap.xml") if count == 1 => {
                self.get_sitemap(get).await.into()
            }
            Some("__css") if count == 2 => self.get_css(get).await.into(),
            _ => self.get_page(get).await.into(),
        };
        if reply.status() == HttpStatus::InternalServerError {
            //TODO reduce information, if not debugging
        }
        //TODO error page
        reply
    }

    pub async fn post(&mut self, get: &CmsGetArgs, post: &CmsPostArgs) -> CmsReply {
        //TODO
        Default::default()
    }
}

// vim: ts=4 sw=4 expandtab

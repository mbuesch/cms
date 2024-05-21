// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::resolver::Resolver;
use anyhow as ah;

#[derive(Clone, Debug)]
pub struct Anchor {
    name: String,
    text: String,
    indent: Option<usize>,
    no_index: bool,
}

impl Anchor {
    pub fn new(name: &str, text: &str, indent: i64, no_index: bool) -> Self {
        let indent = if indent >= 0 {
            Some(indent.clamp(0, u8::MAX.into()).try_into().unwrap())
        } else {
            None
        };
        Self {
            name: name.to_string(),
            text: text.to_string(),
            indent,
            no_index,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn indent(&self) -> Option<usize> {
        self.indent
    }

    pub fn no_index(&self) -> bool {
        self.no_index
    }

    fn make_url(&self, resolver: &Resolver) -> ah::Result<String> {
        let ident = resolver.expand_variable("CMS_PAGEIDENT")?;
        let name = self.name();
        Ok(format!("{ident}#{name}"))
    }

    pub fn make_html(&self, resolver: &Resolver, with_id: bool) -> ah::Result<String> {
        let name = self.name();
        let text = self.text();
        let url = self.make_url(resolver)?;
        if with_id {
            Ok(format!(r#"<a id="{name}" href="{url}">{text}</a>"#))
        } else {
            Ok(format!(r#"<a href="{url}">{text}</a>"#))
        }
    }
}

// vim: ts=4 sw=4 expandtab

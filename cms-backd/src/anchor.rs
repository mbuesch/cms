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

use crate::resolver::Resolver;
use anyhow as ah;

pub struct Anchor {
    name: String,
    text: String,
    indent: Option<usize>,
    no_index: bool,
}

impl Anchor {
    pub fn new(name: &str, text: &str, indent: i64, no_index: bool) -> Self {
        let indent = if indent >= 0 {
            Some(indent.clamp(0, usize::MAX as i64) as usize)
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

    pub fn make_url(&self, resolver: &Resolver) -> ah::Result<String> {
        let ident = resolver.expand_variable("CMS_PAGEIDENT")?;
        let name = self.name();
        Ok(format!("{ident}#{name}"))
    }
}

// vim: ts=4 sw=4 expandtab

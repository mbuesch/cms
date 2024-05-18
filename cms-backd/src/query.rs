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

use crate::numparse::parse_i64;
use std::collections::HashMap;

pub struct Query {
    items: HashMap<String, Vec<u8>>,
}

impl Query {
    pub fn new(items: HashMap<String, Vec<u8>>) -> Self {
        Self { items }
    }

    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        self.items.get(name).cloned()
    }

    pub fn get_str(&self, name: &str) -> Option<String> {
        if let Some(v) = self.get(name) {
            String::from_utf8(v).ok()
        } else {
            None
        }
    }

    pub fn get_int(&self, name: &str) -> Option<i64> {
        if let Some(v) = self.get_str(name) {
            parse_i64(&v).ok()
        } else {
            None
        }
    }
}

// vim: ts=4 sw=4 expandtab

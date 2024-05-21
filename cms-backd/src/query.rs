// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::numparse::parse_i64;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Query {
    items: HashMap<String, Vec<u8>>,
}

impl Query {
    pub fn new(items: HashMap<String, Vec<u8>>) -> Self {
        Self { items }
    }

    pub fn into_items(self) -> HashMap<String, Vec<u8>> {
        self.items
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

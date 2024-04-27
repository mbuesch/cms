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

#![allow(dead_code)] //TODO

use cms_ident::Ident;
use lru::LruCache;
use tokio::sync::Mutex;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum CacheKey {
    //TODO
    Page(Ident),
}

#[derive(Debug)]
enum CacheValue {
    //TODO
    Blob(Vec<u8>),
}

pub struct CmsCache {
    cache: Option<Mutex<LruCache<CacheKey, CacheValue>>>,
}

impl CmsCache {
    pub fn new(cache_size: usize) -> Self {
        let cache = if cache_size == 0 {
            None
        } else {
            let cache_size = cache_size.try_into().unwrap();
            Some(Mutex::new(LruCache::new(cache_size)))
        };
        Self { cache }
    }
}

// vim: ts=4 sw=4 expandtab

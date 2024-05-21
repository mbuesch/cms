// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

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

    pub async fn clear(&self) {
        if let Some(cache) = &self.cache {
            let mut cache = cache.lock().await;
            if !cache.is_empty() {
                cache.clear();
                println!("Backend cache cleared.");
            }
        }
    }
}

// vim: ts=4 sw=4 expandtab

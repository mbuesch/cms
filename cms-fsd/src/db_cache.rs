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

use crate::db_fsintf::{DbFsIntf, PageInfo};
use cms_ident::{CheckedIdent, CheckedIdentElem, Ident};
use inotify::{Inotify, Watches};
use lru::LruCache;
use tokio::sync::Mutex;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum CacheKey {
    Page(Ident),
    PageRedirect(Ident),
    PageTitle(Ident),
    PageStamp(Ident),
    PagePrio(Ident),
    Subpages(Ident),
    NavStop(Ident),
    NavLabel(Ident),
    Macro(Ident, Ident),
    String(Ident),
    Headers(Ident),
}

#[derive(Debug)]
enum CacheValue {
    Blob(Vec<u8>),
    PageInfoList(Vec<PageInfo>),
    Bool(bool),
    U64(u64),
}

pub struct DbCache {
    fs_intf: DbFsIntf,
    inotify: Mutex<Inotify>,
    inotify_watches: Watches,
    cache: Mutex<LruCache<CacheKey, CacheValue>>,
}

macro_rules! get_cached {
    (
        $self:ident,
        $key:ident,
        $value_type:ident,
        $getter:ident ( $( $getter_args:ident ),* )
    ) => {
        {
            let unpack = |data: &CacheValue| {
                if let CacheValue::$value_type(data) = data {
                    data.clone()
                } else {
                    panic!("CacheValue: Not a valid type.");
                }
            };

            // Query the cache.
            {
                let mut cache = $self.cache.lock().await;
                if let Some(data) = cache.get(&$key) {
                    return unpack(data);
                }
                // The cache does not contain the value.
            }

            let data = {
                // Get an inotify handle for adding watches.
                let mut watches = $self.inotify_watches.clone();
                // Access the DB without holding any lock.
                $self.fs_intf.$getter( $( $getter_args ),* , &mut watches ).await
            };

            // Insert it into the cache.
            {
                let mut cache = $self.cache.lock().await;
                unpack(cache.try_get_or_insert::<_, ()>(
                    $key,
                    || Ok(CacheValue::$value_type(data))
                ).unwrap())
            }
        }
    }
}

impl DbCache {
    pub fn new(fs_intf: DbFsIntf, cache_size: u32) -> Self {
        let cache_size: usize = cache_size.try_into().unwrap();
        let cache_size = cache_size.try_into().unwrap();
        let inotify = Inotify::init().expect("Inotify initialization failed");
        let watches = inotify.watches();
        Self {
            fs_intf,
            inotify: Mutex::new(inotify),
            inotify_watches: watches,
            cache: Mutex::new(LruCache::new(cache_size)),
        }
    }

    pub async fn clear(&self) {
        {
            let mut cache = self.cache.lock().await;
            if cache.is_empty() {
                return;
            }
            cache.clear();
        }
        println!("DB cache cleared.");
    }

    pub async fn check_inotify(&self) {
        let mut inotify = self.inotify.lock().await;
        let mut buffer = [0; 4096];
        loop {
            match inotify.read_events(&mut buffer) {
                Ok(events) => {
                    if events.count() > 0 {
                        self.clear().await;
                    } else {
                        return;
                    }
                }
                Err(_) => {
                    return;
                }
            }
        }
    }

    pub async fn get_page(&self, page: &CheckedIdent) -> Vec<u8> {
        let key = CacheKey::Page(page.as_downgrade_ref().clone());
        get_cached!(self, key, Blob, get_page(page))
    }

    pub async fn get_page_redirect(&self, page: &CheckedIdent) -> Vec<u8> {
        let key = CacheKey::PageRedirect(page.as_downgrade_ref().clone());
        get_cached!(self, key, Blob, get_page_redirect(page))
    }

    pub async fn get_page_title(&self, page: &CheckedIdent) -> Vec<u8> {
        let key = CacheKey::PageTitle(page.as_downgrade_ref().clone());
        get_cached!(self, key, Blob, get_page_title(page))
    }

    pub async fn get_page_stamp(&self, page: &CheckedIdent) -> u64 {
        let key = CacheKey::PageStamp(page.as_downgrade_ref().clone());
        get_cached!(self, key, U64, get_page_stamp(page))
    }

    pub async fn get_page_prio(&self, page: &CheckedIdent) -> u64 {
        let key = CacheKey::PagePrio(page.as_downgrade_ref().clone());
        get_cached!(self, key, U64, get_page_prio(page))
    }

    pub async fn get_subpages(&self, page: &CheckedIdent) -> Vec<PageInfo> {
        let key = CacheKey::Subpages(page.as_downgrade_ref().clone());
        get_cached!(self, key, PageInfoList, get_subpages(page))
    }

    pub async fn get_nav_stop(&self, page: &CheckedIdent) -> bool {
        let key = CacheKey::NavStop(page.as_downgrade_ref().clone());
        get_cached!(self, key, Bool, get_nav_stop(page))
    }

    pub async fn get_nav_label(&self, page: &CheckedIdent) -> Vec<u8> {
        let key = CacheKey::NavLabel(page.as_downgrade_ref().clone());
        get_cached!(self, key, Blob, get_nav_label(page))
    }

    pub async fn get_macro(&self, page: &CheckedIdent, name: &CheckedIdentElem) -> Vec<u8> {
        let key = CacheKey::Macro(
            page.as_downgrade_ref().clone(),
            name.as_downgrade_ref().clone(),
        );
        get_cached!(self, key, Blob, get_macro(page, name))
    }

    pub async fn get_string(&self, name: &CheckedIdentElem) -> Vec<u8> {
        let key = CacheKey::String(name.as_downgrade_ref().clone());
        get_cached!(self, key, Blob, get_string(name))
    }

    pub async fn get_headers(&self, page: &CheckedIdent) -> Vec<u8> {
        let key = CacheKey::Headers(page.as_downgrade_ref().clone());
        get_cached!(self, key, Blob, get_headers(page))
    }
}

// vim: ts=4 sw=4 expandtab

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

use anyhow::{self as ah, format_err as err, Context as _};
use cms_ident::{CheckedIdent, CheckedIdentElem, Ident, Strip, Tail};
use inotify::{WatchMask, Watches};
use std::path::{Path, PathBuf};
use tokio::{
    fs::{read_dir, File, OpenOptions},
    io::AsyncReadExt as _,
};

fn elem(e: &'static str) -> CheckedIdentElem {
    // Panic, if the string contains invalid characters.
    let ident = e.parse::<Ident>().unwrap();
    ident.into_checked_element().unwrap()
}

fn syselem(e: &'static str) -> CheckedIdentElem {
    // Panic, if the string contains invalid characters.
    let ident = e.parse::<Ident>().unwrap();
    ident.into_checked_sys_element().unwrap()
}

lazy_static::lazy_static! {
    static ref TAIL_CONTENT_HTML: Tail = Tail::One(elem("content.html"));
    static ref TAIL_HEADER_HTML: Tail = Tail::One(elem("header.html"));
    static ref TAIL_REDIRECT: Tail = Tail::One(elem("redirect"));
    static ref TAIL_TITLE: Tail = Tail::One(elem("title"));
    static ref TAIL_PRIORITY: Tail = Tail::One(elem("priority"));
    static ref TAIL_NAV_STOP: Tail = Tail::One(elem("nav_stop"));
    static ref TAIL_NAV_LABEL: Tail = Tail::One(elem("nav_label"));
    static ref ELEM_MACROS: CheckedIdentElem = syselem("__macros");
    static ref WATCH_MASK: WatchMask =
        WatchMask::CREATE
        | WatchMask::DELETE
        | WatchMask::DELETE_SELF
        | WatchMask::MODIFY
        | WatchMask::MOVE_SELF
        | WatchMask::MOVE
        | WatchMask::ATTRIB;
}

#[inline]
async fn fs_add_dir_watch(path: &Path, watches: &mut Watches) {
    if path.is_dir() {
        let _ = watches.add(path, *WATCH_MASK);
    }
}

#[inline]
async fn fs_add_file_watch(path: &Path, watches: &mut Watches) {
    if path.is_file() {
        let _ = watches.add(path, *WATCH_MASK);
    }
}

#[inline]
async fn fs_file_open_r(path: &Path, watches: &mut Watches) -> ah::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .await
        .context("Open database file")?;

    fs_add_file_watch(path, watches).await;
    if let Some(parent_dir) = path.parent() {
        fs_add_dir_watch(parent_dir, watches).await;
    }

    Ok(file)
}

#[inline]
async fn fs_file_mtime(path: &Path, watches: &mut Watches) -> ah::Result<u64> {
    let fd = fs_file_open_r(path, watches).await?;
    let mtime = fd
        .metadata()
        .await
        .context("Get database file metadata")?
        .modified()
        .context("Get database file mtime")?;
    let mtime = mtime
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .context("Convert mtime to unix time")?;
    Ok(mtime.as_secs())
}

#[inline]
async fn fs_file_read(path: &Path, watches: &mut Watches) -> ah::Result<Vec<u8>> {
    let mut fd = fs_file_open_r(path, watches).await?;
    let mut buf = vec![]; // read_to_end will allocate before read.
    fd.read_to_end(&mut buf)
        .await
        .context("Read database file")?;
    Ok(buf)
}

#[inline]
async fn fs_file_is_empty(path: &Path, watches: &mut Watches) -> ah::Result<bool> {
    Ok(fs_file_read(path, watches).await?.is_empty())
}

#[inline]
async fn fs_file_read_string(path: &Path, watches: &mut Watches) -> ah::Result<String> {
    let data = fs_file_read(path, watches).await?;
    String::from_utf8(data).context("Database file UTF-8 encoding")
}

#[inline]
async fn fs_file_read_u64(path: &Path, watches: &mut Watches) -> ah::Result<u64> {
    fs_file_read_string(path, watches)
        .await?
        .trim()
        .parse::<u64>()
        .context("Database parse u64 value")
}

#[inline]
async fn fs_file_read_bool(path: &Path, watches: &mut Watches) -> ah::Result<bool> {
    let value = fs_file_read_u64(path, watches).await?;
    Ok(value != 0)
}

#[derive(Clone, Debug)]
pub struct PageInfo {
    pub name: Vec<u8>,
    pub nav_label: Vec<u8>,
    pub prio: u64,
}

pub struct DbFsIntf {
    db_pages: PathBuf,
    db_macros: PathBuf,
    db_strings: PathBuf,
}

impl DbFsIntf {
    const DEFAULT_PRIO: u64 = 500;
    const DEFAULT_MTIME: u64 = 0;

    pub fn new(path: &Path) -> ah::Result<Self> {
        if !path.is_dir() {
            return Err(err!("DB: {:?} is not a directory.", path));
        }
        let db_pages = path.join("pages");
        if !db_pages.is_dir() {
            return Err(err!("DB: {:?} is not a directory.", db_pages));
        }
        let db_macros = path.join("macros");
        if !db_macros.is_dir() {
            return Err(err!("DB: {:?} is not a directory.", db_macros));
        }
        let db_strings = path.join("strings");
        if !db_strings.is_dir() {
            return Err(err!("DB: {:?} is not a directory.", db_strings));
        }
        Ok(Self {
            db_pages,
            db_macros,
            db_strings,
        })
    }

    pub async fn get_page(&self, page: &CheckedIdent, watches: &mut Watches) -> Vec<u8> {
        let path = page.to_fs_path(&self.db_pages, &TAIL_CONTENT_HTML);
        fs_file_read(&path, watches)
            .await
            .unwrap_or_else(|_| vec![])
    }

    pub async fn get_page_redirect(&self, page: &CheckedIdent, watches: &mut Watches) -> Vec<u8> {
        let path = page.to_fs_path(&self.db_pages, &TAIL_REDIRECT);
        fs_file_read(&path, watches)
            .await
            .unwrap_or_else(|_| vec![])
    }

    pub async fn get_page_title(&self, page: &CheckedIdent, watches: &mut Watches) -> Vec<u8> {
        let path = page.to_fs_path(&self.db_pages, &TAIL_TITLE);
        if let Ok(title) = fs_file_read(&path, watches).await {
            title
        } else {
            self.get_nav_label(page, watches).await
        }
    }

    pub async fn get_page_stamp(&self, page: &CheckedIdent, watches: &mut Watches) -> u64 {
        let path = page.to_fs_path(&self.db_pages, &TAIL_CONTENT_HTML);
        fs_file_mtime(&path, watches)
            .await
            .unwrap_or(Self::DEFAULT_MTIME)
    }

    pub async fn get_page_prio(&self, page: &CheckedIdent, watches: &mut Watches) -> u64 {
        let path = page.to_fs_path(&self.db_pages, &TAIL_PRIORITY);
        fs_file_read_u64(&path, watches)
            .await
            .unwrap_or(Self::DEFAULT_PRIO)
    }

    pub async fn get_subpages(&self, page: &CheckedIdent, watches: &mut Watches) -> Vec<PageInfo> {
        let path = page.to_fs_path(&self.db_pages, &Tail::None);
        let mut subpages = Vec::with_capacity(64);
        fs_add_dir_watch(&path, watches).await;
        if let Ok(mut dir_reader) = read_dir(path).await {
            while let Ok(Some(entry)) = dir_reader.next_entry().await {
                let epath = entry.path();
                let ename = entry.file_name();

                if ename.as_encoded_bytes().starts_with(b".") {
                    continue; // No . and ..
                }
                if ename.as_encoded_bytes().starts_with(b"__") {
                    continue; // No system folders and files.
                }
                if !epath.is_dir() {
                    continue; // Not a directory.
                }
                if epath.join("hidden").exists() {
                    continue; // This entry is hidden.
                }
                if !fs_file_is_empty(&epath.join("redirect"), watches)
                    .await
                    .unwrap_or(true)
                {
                    continue; // This entry is redirected to somewhere else.
                }
                let Some(ename_str) = ename.to_str() else {
                    continue; // Entry name is not a valid str.
                };
                let Ok(subpage_ident) = page.clone_append(ename_str).into_checked() else {
                    continue; // Entry name is not a valid CheckedIdent element.
                };

                let nav_label = self.get_nav_label(&subpage_ident, watches).await;
                let prio = self.get_page_prio(&subpage_ident, watches).await;
                let info = PageInfo {
                    name: ename.into_encoded_bytes(),
                    nav_label,
                    prio,
                };
                subpages.push(info);

                fs_add_dir_watch(&epath, watches).await;
            }
        }
        subpages
    }

    pub async fn get_nav_stop(&self, page: &CheckedIdent, watches: &mut Watches) -> bool {
        let path = page.to_fs_path(&self.db_pages, &TAIL_NAV_STOP);
        fs_file_read_bool(&path, watches).await.unwrap_or(false)
    }

    pub async fn get_nav_label(&self, page: &CheckedIdent, watches: &mut Watches) -> Vec<u8> {
        let path = page.to_fs_path(&self.db_pages, &TAIL_NAV_LABEL);
        fs_file_read(&path, watches)
            .await
            .unwrap_or_else(|_| vec![])
    }

    pub async fn get_macro(
        &self,
        page: &CheckedIdent,
        name: &CheckedIdentElem,
        watches: &mut Watches,
    ) -> Vec<u8> {
        // Try to get the page specific macro.
        // Traverse the path backwards.
        let tail = Tail::Two(ELEM_MACROS.clone(), name.clone());
        let mut rstrip = 0;
        while let Ok(path) = page.to_stripped_fs_path(&self.db_pages, Strip::Right(rstrip), &tail) {
            if let Ok(data) = fs_file_read(&path, watches).await {
                return data;
            }
            rstrip += 1;
        }

        // Try to get the global macro.
        let path = name.to_fs_path(&self.db_macros, &Tail::None);
        fs_file_read(&path, watches)
            .await
            .unwrap_or_else(|_| vec![])
    }

    pub async fn get_string(&self, name: &CheckedIdentElem, watches: &mut Watches) -> Vec<u8> {
        let path = name.to_fs_path(&self.db_strings, &Tail::None);
        fs_file_read(&path, watches)
            .await
            .unwrap_or_else(|_| vec![])
    }

    pub async fn get_headers(&self, page: &CheckedIdent, watches: &mut Watches) -> Vec<u8> {
        let mut ret = Vec::with_capacity(4096);
        let mut rstrip = 0;
        while let Ok(path) =
            page.to_stripped_fs_path(&self.db_pages, Strip::Right(rstrip), &TAIL_HEADER_HTML)
        {
            if let Ok(data) = fs_file_read(&path, watches).await {
                ret.extend_from_slice(&data);
            }
            rstrip += 1;
        }
        ret
    }
}

// vim: ts=4 sw=4 expandtab

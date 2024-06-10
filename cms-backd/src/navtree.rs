// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::comm::{CmsComm, CommGetPage, CommPage, CommSubPages};
use async_recursion::async_recursion;
use cms_ident::CheckedIdent;
use std::cmp::Ordering;

const MAX_DEPTH: usize = 64;

fn elem_sort_cmp(a: &NavElem, b: &NavElem) -> Ordering {
    // compare a(prio|nav_label.lower) to b(prio|nav_label.lower)
    if a.prio() == b.prio() {
        let a = a.nav_label().trim().to_lowercase();
        let b = b.nav_label().trim().to_lowercase();
        a.cmp(&b)
    } else {
        a.prio().cmp(&b.prio())
    }
}

#[derive(Clone, Debug)]
pub struct NavElem {
    name: String,
    nav_label: String,
    path: CheckedIdent,
    prio: u64,
    active: bool,
    children: Vec<NavElem>,
}

impl NavElem {
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn nav_label(&self) -> &str {
        &self.nav_label
    }

    pub fn path(&self) -> &CheckedIdent {
        &self.path
    }

    pub fn prio(&self) -> u64 {
        self.prio
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn children(&self) -> &[NavElem] {
        &self.children
    }
}

#[derive(Clone, Debug)]
pub struct NavTree {
    tree: Vec<NavElem>,
}

impl NavTree {
    pub async fn build(
        comm: &mut CmsComm,
        root_page: &CheckedIdent,
        active_page: Option<&CheckedIdent>,
    ) -> Self {
        let tree = Self::build_sub(comm, root_page, active_page, 0).await;
        Self { tree }
    }

    #[async_recursion]
    async fn build_sub(
        comm: &mut CmsComm,
        base: &CheckedIdent,
        active: Option<&CheckedIdent>,
        depth: usize,
    ) -> Vec<NavElem> {
        if depth >= MAX_DEPTH {
            return vec![];
        }

        let Ok(CommPage { nav_stop, .. }) = comm
            .get_db_page(CommGetPage {
                path: base.clone(),
                get_nav_stop: true,
                ..Default::default()
            })
            .await
        else {
            return vec![];
        };
        if nav_stop.unwrap_or(true) {
            return vec![];
        }

        let Ok(CommSubPages {
            names,
            nav_labels,
            prios,
        }) = comm.get_db_sub_pages(base).await
        else {
            return vec![];
        };
        let count = names.len();

        let mut ret = Vec::with_capacity(count);
        for i in 0..count {
            let sub_nav_label = &nav_labels[i];
            if sub_nav_label.trim().is_empty() {
                continue;
            }
            let sub_name = &names[i];
            let Ok(sub_ident) = base.clone_append(sub_name).into_checked() else {
                continue;
            };
            let sub_prio = prios[i];
            let sub_active = active
                .map(|a| a.starts_with(sub_ident.as_downgrade_ref()))
                .unwrap_or(false);

            let sub_children = Self::build_sub(comm, &sub_ident, active, depth + 1).await;

            ret.push(NavElem {
                name: sub_name.clone(),
                nav_label: sub_nav_label.clone(),
                path: sub_ident,
                prio: sub_prio,
                active: sub_active,
                children: sub_children,
            });
        }
        ret.sort_unstable_by(elem_sort_cmp);
        ret
    }

    pub fn elems(&self) -> &[NavElem] {
        &self.tree
    }
}

// vim: ts=4 sw=4 expandtab

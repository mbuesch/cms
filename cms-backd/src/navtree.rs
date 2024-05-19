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

use crate::comm::{CmsComm, CommGetPage, CommPage, CommSubPages};
use async_recursion::async_recursion;
use cms_ident::CheckedIdent;

const MAX_DEPTH: usize = 64;

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
        ret
    }

    pub fn elems(&self) -> &[NavElem] {
        &self.tree
    }
}

// vim: ts=4 sw=4 expandtab

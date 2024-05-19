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

#![forbid(unsafe_code)]

use cms_ident::Ident;
use cms_socket::impl_msg_serde;
use serde::{Deserialize, Serialize};

pub const SOCK_FILE: &str = "cms-fsd.sock";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Msg {
    // Getters
    GetPage {
        path: Ident,
        get_title: bool,
        get_data: bool,
        get_stamp: bool,
        get_prio: bool, //TODO remove this
        get_redirect: bool,
        get_nav_stop: bool,  //TODO move this to GetSubPages
        get_nav_label: bool, //TODO remove this
    },
    GetHeaders {
        path: Ident,
    },
    GetSubPages {
        path: Ident,
    },
    GetMacro {
        parent: Ident,
        name: Ident,
    },
    GetString {
        name: Ident,
    },
    GetImage {
        name: Ident,
    },

    // Values
    Page {
        title: Option<Vec<u8>>,
        data: Option<Vec<u8>>,
        stamp: Option<u64>,
        prio: Option<u64>,
        redirect: Option<Vec<u8>>,
        nav_stop: Option<bool>,
        nav_label: Option<Vec<u8>>,
    },
    Headers {
        data: Vec<u8>,
    },
    SubPages {
        names: Vec<Vec<u8>>,
        nav_labels: Vec<Vec<u8>>,
        prios: Vec<u64>,
    },
    Macro {
        data: Vec<u8>,
    },
    String {
        data: Vec<u8>,
    },
    Image {
        data: Vec<u8>,
    },
}

impl_msg_serde!(Msg, 0x8F5755D6);

// vim: ts=4 sw=4 expandtab

// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

use cms_ident::Ident;
use cms_socket::impl_msg_serde;
use rkyv::{Archive, Deserialize, Serialize};

pub const SOCK_FILE: &str = "cms-fsd.sock";

#[derive(Serialize, Deserialize, Archive, Clone, Debug)]
pub enum Msg {
    // Getters
    GetPage {
        path: Ident,
        get_title: bool,
        get_data: bool,
        get_stamp: bool,
        get_redirect: bool,
    },
    GetHeaders {
        path: Ident,
    },
    GetSubPages {
        path: Ident,
        get_nav_labels: bool,
        get_nav_stops: bool,
        get_stamps: bool,
        get_prios: bool,
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
        redirect: Option<Vec<u8>>,
    },
    Headers {
        data: Vec<u8>,
    },
    SubPages {
        names: Vec<Vec<u8>>,
        nav_labels: Vec<Vec<u8>>,
        nav_stops: Vec<bool>,
        stamps: Vec<u64>,
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

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
use std::collections::HashMap;

pub const SOCK_FILE: &str = "cms-backd.sock";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Msg {
    Get {
        host: Vec<u8>,
        path: Ident,
        https: bool,
        cookie: Vec<u8>,
        query: HashMap<String, Vec<u8>>,
    },
    Post {
        host: Vec<u8>,
        path: Ident,
        https: bool,
        cookie: Vec<u8>,
        query: HashMap<String, Vec<u8>>,
        body: Vec<u8>,
        body_mime: Vec<u8>,
    },
    Reply {
        error: String,
        body: Vec<u8>,
        mime: String,
    },
}

impl_msg_serde!(Msg, 0x9C66EA74);

// vim: ts=4 sw=4 expandtab

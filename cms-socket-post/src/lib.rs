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
use std::collections::HashMap;

pub const SOCK_FILE: &str = "cms-postd.sock";

#[derive(Serialize, Deserialize, Archive, Clone, Debug)]
pub enum Msg {
    // Getters
    RunPostHandler {
        path: Ident,
        query: HashMap<String, Vec<u8>>,
        form_fields: HashMap<String, Vec<u8>>,
    },

    // Values
    PostHandlerResult {
        error: String,
        body: Vec<u8>,
        mime: String,
    },
}

impl_msg_serde!(Msg, 0x6ADCB73F);

// vim: ts=4 sw=4 expandtab

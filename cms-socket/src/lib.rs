// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2025 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

mod msg;
mod sock_async;
mod sock_sync;

pub use crate::{
    msg::{DeserializeResult, MSG_HDR_LEN, MsgHdr, MsgSerde},
    sock_async::{CmsSocket, CmsSocketConn},
    sock_sync::CmsSocketConnSync,
};

// vim: ts=4 sw=4 expandtab

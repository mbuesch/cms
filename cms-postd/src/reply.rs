// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub struct Reply {
    pub error: String,
    pub body: Vec<u8>,
    pub mime: String,
}

// vim: ts=4 sw=4 expandtab

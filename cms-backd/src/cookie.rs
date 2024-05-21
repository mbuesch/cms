// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub struct Cookie {
    _data: Vec<u8>,
}

impl Cookie {
    pub fn new(data: Vec<u8>) -> Self {
        Self { _data: data }
    }
}

// vim: ts=4 sw=4 expandtab

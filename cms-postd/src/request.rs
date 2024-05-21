// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use cms_ident::CheckedIdent;
use std::collections::HashMap;

pub struct Request {
    pub path: CheckedIdent,
    pub query: HashMap<String, Vec<u8>>,
    pub form_fields: HashMap<String, Vec<u8>>,
}

// vim: ts=4 sw=4 expandtab

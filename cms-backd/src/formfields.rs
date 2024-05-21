// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::{self as ah, Context as _};
use multer::{parse_boundary, Constraints, Multipart, SizeLimit};
use std::collections::HashMap;

const LIMIT_WHOLE_STREAM: u64 = 1024 * 1024;
const LIMIT_PER_FIELD: u64 = 1024 * 128;

pub struct FormFields {
    items: HashMap<String, Vec<u8>>,
}

impl FormFields {
    pub async fn new(body: &[u8], body_mime: &str) -> ah::Result<Self> {
        let boundary = parse_boundary(body_mime).context("Parse form-data boundary")?;
        let sizelim = SizeLimit::new()
            .whole_stream(LIMIT_WHOLE_STREAM)
            .per_field(LIMIT_PER_FIELD);
        let constr = Constraints::new().size_limit(sizelim);
        let mut multipart = Multipart::with_reader_with_constraints(body, boundary, constr);
        let mut items = HashMap::new();
        while let Some(field) = multipart.next_field().await.context("Multipart field")? {
            let Some(name) = field.name() else {
                continue;
            };
            let name = name.to_string();
            let Ok(data) = field.bytes().await else {
                continue;
            };
            let data = data.to_vec();
            items.insert(name, data);
        }
        Ok(Self { items })
    }

    pub fn into_items(self) -> HashMap<String, Vec<u8>> {
        self.items
    }
}

// vim: ts=4 sw=4 expandtab

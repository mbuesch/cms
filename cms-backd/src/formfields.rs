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

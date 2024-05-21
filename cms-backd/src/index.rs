// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[derive(Clone, Debug)]
pub struct IndexRef {
    char_index: usize,
}

impl IndexRef {
    pub fn new(char_index: usize) -> Self {
        Self { char_index }
    }

    pub fn char_index(&self) -> usize {
        self.char_index
    }
}

// vim: ts=4 sw=4 expandtab

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

use std::{collections::HashMap, rc::Rc};
use crunchy::unroll;

pub type VarName<'a> = &'a str;
pub type VarFn<'a> = Rc<dyn Fn(&str) -> String + Send + Sync + 'a>;

macro_rules! getvar {
    ($expression:expr) => {
        Rc::new(|_| $expression)
    };
}
pub(crate) use getvar;

const ESCAPE_CHARS: [char; 6] = ['\\', ',', '@', '$', '(', ')'];

pub struct ResolverVars<'a> {
    vars: HashMap<VarName<'a>, VarFn<'a>>,
    prefixes: HashMap<VarName<'a>, VarFn<'a>>,
}

impl<'a> ResolverVars<'a> {
    pub fn new() -> Self {
        let mut this = Self {
            vars: HashMap::with_capacity(32),
            prefixes: HashMap::with_capacity(8),
        };
        this.register("BR", getvar!("<br />".to_string()));
        this
    }

    pub fn register(&mut self, name: VarName<'a>, fun: VarFn<'a>) {
        self.vars.insert(name, fun);
    }

    pub fn register_prefix(&mut self, prefix: VarName<'a>, fun: VarFn<'a>) {
        self.prefixes.insert(prefix, fun);
    }

    pub fn get(&self, name: VarName<'_>) -> String {
        // Find normal variable.
        if let Some(fun) = self.vars.get(name) {
            // Call the getter.
            return Resolver::escape(&fun(name));
        }
        // Find variable by prefix.
        if let Some(index) = name.find('_') {
            if index > 0 {
                if let Some(fun) = self.prefixes.get(&name[..index]) {
                    // Call the getter.
                    return Resolver::escape(&fun(name));
                }
            }
        }
        // No variable found.
        Default::default()
    }
}

pub struct Resolver<'a> {
    vars: &'a ResolverVars<'a>,
}

impl<'a> Resolver<'a> {
    pub fn escape(text: &str) -> String {
        let mut escaped = String::with_capacity(text.len() * 2);
        'mainloop: for c in text.chars() {
            debug_assert_eq!(ESCAPE_CHARS.len(), 6);
            unroll! {
                for i in 0..6 {
                    if c == ESCAPE_CHARS[i] {
                        escaped.push('\\');
                        escaped.push(c);
                        continue 'mainloop;
                    }
                }
            }
            escaped.push(c);
        }
        escaped
    }

    pub fn unescape(text: &str) -> String {
        let mut unescaped = String::with_capacity(text.len());
        let mut text_chars = text.chars();
        while let Some(c) = text_chars.next() {
            if c == '\\' {
                if let Some(nc) = text_chars.next() {
                    unescaped.push(nc);
                }
            } else {
                unescaped.push(c);
            }
        }
        unescaped
    }

    pub fn new(vars: &'a ResolverVars<'a>) -> Self {
        Self { vars }
    }

    pub fn run(&mut self, input: &str) -> String {
        input.to_string() //TODO
    }
}

// vim: ts=4 sw=4 expandtab

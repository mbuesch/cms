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

pub type VarName<'a> = &'a str;
pub type VarFn<'a> = Rc<dyn Fn() -> String + Send + Sync + 'a>;

macro_rules! getvar {
    ($expression:expr) => {
        Rc::new(|| $expression)
    };
}
pub(crate) use getvar;

pub struct ResolverVars<'a> {
    vars: HashMap<VarName<'a>, VarFn<'a>>,
}

impl<'a> ResolverVars<'a> {
    pub fn new() -> Self {
        let mut this = Self {
            vars: HashMap::with_capacity(32),
        };
        this.register("BR", getvar!("<br />".to_string()));
        this
    }

    pub fn register(&mut self, name: VarName<'a>, fun: VarFn<'a>) {
        self.vars.insert(name, fun);
    }
}

pub struct Resolver<'a> {
    vars: &'a ResolverVars<'a>,
}

impl<'a> Resolver<'a> {
    pub fn new(vars: &'a ResolverVars<'a>) -> Self {
        Self { vars }
    }

    pub fn run(&mut self, input: &str) -> String {
        input.to_string() //TODO
    }
}

// vim: ts=4 sw=4 expandtab

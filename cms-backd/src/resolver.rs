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

use crate::itertools::{iter_cons_until, iter_cons_until_in, iter_cons_until_not_in};
use crunchy::unroll;
use multipeek::IteratorExt as _;
use std::{collections::HashMap, rc::Rc};

pub type VarName<'a> = &'a str;
pub type VarFn<'a> = Rc<dyn Fn(&str) -> String + Send + Sync + 'a>;

macro_rules! getvar {
    ($expression:expr) => {
        Rc::new(|_| $expression)
    };
}
pub(crate) use getvar;

const ESCAPE_CHARS: [char; 6] = ['\\', ',', '@', '$', '(', ')'];
const NUMBER_CHARS: [char; 10] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
const VARNAME_CHARS: [char; 27] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '_',
];

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

    fn expand_stmts(&mut self, data: &str, stop_chars: &[char]) -> String {
        let mut exp = String::with_capacity(data.len() * 2);

        let mut chars = data.chars().multipeek();
        'mainloop: while let Some(c) = chars.next() {
            let mut res: Option<String> = None;
            match c {
                '\\' if chars
                    .peek_nth(0)
                    .map(|c| ESCAPE_CHARS.contains(c))
                    .unwrap_or(false) =>
                {
                    // Escaped characters
                    // Keep escapes. They are removed later.
                    let mut r = String::with_capacity(2);
                    r.push(c);
                    r.push(chars.next().unwrap());
                    res = Some(r);
                }
                '\n' => {
                    // Newline
                    //TODO
                }
                '<' if chars.peek_nth(0) == Some(&'!')
                    && chars.peek_nth(1) == Some(&'-')
                    && chars.peek_nth(2) == Some(&'-')
                    && chars.peek_nth(3) == Some(&'-') =>
                {
                    // Comment
                    //TODO
                }
                _ if stop_chars.contains(&c) => {
                    // Stop character
                    break 'mainloop;
                }
                '@' => {
                    // Macro call
                    match iter_cons_until(&mut chars, '(') {
                        Ok(macro_name) => {
                            //TODO
                        }
                        Err(tail) => res = Some(tail),
                    }
                }
                '$' if chars.peek_nth(0).map(|c| c.is_numeric()).unwrap_or(false) => {
                    // Macro argument
                    match iter_cons_until_not_in(&mut chars, &NUMBER_CHARS) {
                        Ok(arg_name) => {
                            //TODO
                        }
                        Err(tail) => res = Some(tail),
                    }
                }
                '$' if chars.peek_nth(0) == Some(&'(') => {
                    // Statement
                    match iter_cons_until_in(&mut chars, &[' ', ')']) {
                        Ok(stmt_name) => {
                            //TODO
                        }
                        Err(tail) => res = Some(tail),
                    }
                }
                '$' => {
                    // Variable
                    match iter_cons_until_not_in(&mut chars, &VARNAME_CHARS) {
                        Ok(var_name) => {
                            //TODO
                        }
                        Err(tail) => res = Some(tail),
                    }
                }
                _ => (),
            }
            if let Some(res) = res {
                //TODO
            } else {
                exp.push(c);
            }
        }
        exp
    }

    pub fn run(mut self, input: &str) -> String {
        if input.is_empty() {
            return String::new();
        }
        let data = self.expand_stmts(input, &[]);
        //TODO indices
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape() {
        let a = "";
        let b = "";
        assert_eq!(Resolver::escape(a), b);

        let a = "\\,@$()";
        let b = "\\\\\\,\\@\\$\\(\\)";
        assert_eq!(Resolver::escape(a), b);

        let a = "abc\\def,@$x(x)x";
        let b = "abc\\\\def\\,\\@\\$x\\(x\\)x";
        assert_eq!(Resolver::escape(a), b);

        let a = "abc\\\\def\\,\\@\\$\\(\\)";
        let b = "abc\\def,@$()";
        assert_eq!(Resolver::unescape(a), b);

        let a = "abc\\"; // dangling escape
        let b = "abc";
        assert_eq!(Resolver::unescape(a), b);

        let a = "\\,@$()abc";
        let b = Resolver::escape(&Resolver::escape(&Resolver::escape(a)));
        let b = Resolver::unescape(&Resolver::unescape(&Resolver::unescape(&b)));
        assert_eq!(a, b);
    }
}

// vim: ts=4 sw=4 expandtab

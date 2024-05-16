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

use crate::{
    backend::CmsComm,
    itertools::{iter_cons_until, iter_cons_until_in, iter_cons_until_not_in},
};
use anyhow::{self as ah, format_err as err};
use async_recursion::async_recursion;
use cms_ident::{CheckedIdent, Ident};
use crunchy::unroll;
use peekable_fwd_bwd::Peekable;
use rand::prelude::*;
use std::{collections::HashMap, sync::Arc};

pub type VarName<'a> = &'a str;
pub type VarFn<'a> = Arc<dyn Fn(&str) -> String + Send + Sync + 'a>;

macro_rules! getvar {
    ($expression:expr) => {
        Arc::new(|_| $expression)
    };
}
pub(crate) use getvar;

const ESCAPE_CHARS: [char; 6] = ['\\', ',', '@', '$', '(', ')'];
const NUMBER_CHARS: [char; 10] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
const VARNAME_CHARS: [char; 27] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '_',
];
const MACRO_STACK_SIZE_ALLOC: usize = 16;
const MACRO_STACK_SIZE_MAX: usize = 128;
const MACRO_NAME_SIZE_MAX: usize = 64;
const NUM_ARGS_MAX: usize = 128;
const NUM_ARGS_ALLOC: usize = 16;
const NUM_ARG_RECURSION_MAX: usize = 128;
const EXPAND_CAPACITY_DEF: usize = 4096;

fn parse_usize(s: &str) -> ah::Result<usize> {
    let s = s.trim();
    if let Some(s) = s.strip_prefix("0x") {
        Ok(usize::from_str_radix(s, 16)?)
    } else {
        Ok(s.parse::<usize>()?)
    }
}

fn parse_i64(s: &str) -> ah::Result<i64> {
    let s = s.trim();
    if let Some(s) = s.strip_prefix("0x") {
        Ok(i64::from_str_radix(s, 16)?)
    } else {
        Ok(s.parse::<i64>()?)
    }
}

fn parse_f64(s: &str) -> ah::Result<f64> {
    Ok(s.trim().parse::<f64>()?)
}

type Chars<'a> = Peekable<std::str::Chars<'a>, 2, 4>;

struct IndexRef {
    char_index: usize,
}

impl IndexRef {
    pub fn new(char_index: usize) -> Self {
        Self { char_index }
    }
}

struct ResolverStackElem {
    lineno: u32,
    name: String,
    args: Vec<String>,
}

impl ResolverStackElem {
    pub fn new(lineno: u32, name: &str, args: Vec<String>) -> Self {
        Self {
            lineno,
            name: name.to_string(),
            args,
        }
    }

    pub fn lineno(&self) -> u32 {
        self.lineno
    }

    pub fn set_lineno(&mut self, lineno: u32) {
        self.lineno = lineno;
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn get_arg(&self, index: usize) -> &str {
        if index < self.args.len() {
            self.args[index].trim()
        } else {
            ""
        }
    }
}

struct ResolverStack {
    elems: Vec<ResolverStackElem>,
}

impl ResolverStack {
    pub fn new() -> Self {
        let mut elems = Vec::with_capacity(MACRO_STACK_SIZE_ALLOC);
        elems.push(ResolverStackElem::new(1, "content.html", vec![]));
        Self { elems }
    }

    pub fn len(&self) -> usize {
        self.elems.len()
    }

    pub fn push(&mut self, elem: ResolverStackElem) {
        self.elems.push(elem);
    }

    pub fn pop(&mut self) -> Option<ResolverStackElem> {
        assert!(self.elems.len() > 1); // must not pop the last element.
        self.elems.pop()
    }

    pub fn top(&self) -> &ResolverStackElem {
        let len = self.elems.len();
        &self.elems[len - 1]
    }

    pub fn top_mut(&mut self) -> &mut ResolverStackElem {
        let len = self.elems.len();
        &mut self.elems[len - 1]
    }
}

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
    comm: &'a mut CmsComm,
    parent: &'a CheckedIdent,
    vars: &'a ResolverVars<'a>,
    stack: ResolverStack,
    args_recursion: usize,
    char_index: usize,
    index_refs: Vec<IndexRef>,
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

    pub fn new(
        comm: &'a mut CmsComm,
        parent: &'a CheckedIdent,
        vars: &'a ResolverVars<'a>,
    ) -> Self {
        Self {
            comm,
            parent,
            vars,
            stack: ResolverStack::new(),
            args_recursion: 0,
            char_index: 0,
            index_refs: vec![],
        }
    }

    fn next(&mut self, chars: &mut Chars<'_>) -> Option<char> {
        if let Some(c) = chars.next() {
            if c == '\n' {
                let top = self.stack.top_mut();
                top.set_lineno(top.lineno() + 1);
            }
            Some(c)
        } else {
            None
        }
    }

    fn stmterr(&self, msg: &str) -> ah::Result<String> {
        let e = if true
        /* TODO */
        {
            let top = self.stack.top();
            err!("{}:{}: {}", top.name(), top.lineno(), msg)
        } else {
            err!("{msg}")
        };
        Err(e)
    }

    #[async_recursion]
    async fn parse_args(&mut self, chars: &mut Chars<'_>) -> ah::Result<Vec<String>> {
        if self.args_recursion > NUM_ARG_RECURSION_MAX {
            self.stmterr("Argument parsing recursion too deep")?;
            unreachable!();
        }
        let mut ret = Vec::with_capacity(NUM_ARGS_ALLOC);
        if chars.peek_bwd() == Some(&')') {
            // no arg
            ret.push("".to_string());
        } else {
            while chars.peek().is_some() {
                if ret.len() >= NUM_ARGS_MAX {
                    self.stmterr("Too many arguments")?;
                    unreachable!();
                }
                self.args_recursion += 1;
                let arg = self.expand(chars, &[',', ')']).await?;
                self.args_recursion -= 1;
                ret.push(arg);
                if chars.peek_bwd() == Some(&')') {
                    break;
                }
            }
        }
        Ok(ret)
    }

    #[async_recursion]
    async fn do_macro(
        &mut self,
        macro_name_str: &str,
        chars: &mut Chars<'_>,
    ) -> ah::Result<String> {
        if self.stack.len() > MACRO_STACK_SIZE_MAX {
            return self.stmterr("Macro stack overflow");
        }

        if macro_name_str.len() > MACRO_NAME_SIZE_MAX {
            return self.stmterr("Macro name is too long");
        }
        let Ok(macro_name) = macro_name_str.parse::<Ident>() else {
            return self.stmterr("Macro name is invalid");
        };
        let Ok(macro_name) = macro_name.into_checked_element() else {
            return self.stmterr("Macro name contains invalid characters");
        };

        let args = self.parse_args(chars).await?;
        let data = self
            .comm
            .get_db_macro(Some(self.parent), &macro_name)
            .await?;

        // Remove empty lines
        let mut cleaned_data = String::with_capacity(data.len());
        let mut first = true;
        for line in data.lines() {
            if !line.trim().is_empty() {
                if !first {
                    cleaned_data.push('\n');
                }
                cleaned_data.push_str(line);
                first = false;
            }
        }

        let mut data = Chars::new(cleaned_data.chars());
        let el = ResolverStackElem::new(1, macro_name_str, args);

        self.stack.push(el);
        let data = self.expand(&mut data, &[]).await?;
        self.stack.pop();

        Ok(data)
    }

    fn expand_macro_arg(&self, arg_name: &str) -> ah::Result<String> {
        let top = self.stack.top();
        let arg_idx = parse_usize(arg_name)?;
        if arg_idx == 0 {
            Ok(top.name().to_string())
        } else {
            Ok(top.get_arg(arg_idx - 1).to_string())
        }
    }

    /// Evaluate a CONDITION and return THEN or ELSE based on the CONDITION.
    /// If ELSE is not specified, then this statement uses an empty string instead of ELSE.
    ///
    /// Statement: $(if CONDITION, THEN, ELSE)
    /// Statement: $(if CONDITION, THEN)
    ///
    /// Returns: THEN if CONDITION is not empty after stripping whitespace.
    /// Returns: ELSE otherwise.
    async fn expand_statement_if(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs != 2 && nargs != 3 {
            return self.stmterr(&format!("IF: invalid number of arguments ({nargs})"));
        }
        let condition = &args[0];
        let b_then = &args[1];
        let b_else = if nargs == 3 { &args[2] } else { "" };
        let result = if condition.trim().is_empty() {
            b_else
        } else {
            b_then
        };
        Ok(result.to_string())
    }

    async fn expand_statement_eq_ne(
        &mut self,
        chars: &mut Chars<'_>,
        ne: bool,
    ) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs < 2 {
            let s = if ne { "NE" } else { "EQ" };
            return self.stmterr(&format!("{s}: invalid args"));
        }
        let all_equal = args
            .iter()
            .map(|a| Some(a.trim()))
            .reduce(|a, b| if a == b { a } else { None })
            .unwrap()
            .is_some();
        let cond = if ne { !all_equal } else { all_equal };
        let result = if cond {
            "1".to_string()
        } else {
            "".to_string()
        };
        Ok(result)
    }

    /// Compares two or more strings for equality.
    ///
    /// Statement: $(eq A, B, ...)
    ///
    /// Returns: 1, if all stripped arguments are equal.
    /// Returns: An empty string otherwise.
    async fn expand_statement_eq(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        self.expand_statement_eq_ne(chars, false).await
    }

    /// Compares two or more strings for inequality.
    ///
    /// Statement: $(ne A, B, ...)
    ///
    /// Returns: 1, if not all stripped arguments are equal.
    /// Returns: An empty string otherwise.
    async fn expand_statement_ne(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        self.expand_statement_eq_ne(chars, true).await
    }

    /// Compares all arguments with logical AND operation.
    ///
    /// Statement: $(and A, B, ...)
    ///
    /// Returns: The first stripped argument (A), if all stripped arguments are non-empty strings.
    /// Returns: An empty string otherwise.
    async fn expand_statement_and(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs < 2 {
            return self.stmterr("AND: invalid args");
        }
        let all_nonempty = args.iter().all(|a| !a.trim().is_empty());
        let result = if all_nonempty { &args[0] } else { "" };
        Ok(result.to_string())
    }

    /// Compares all arguments with logical OR operation.
    ///
    /// Statement: $(or A, B, ...)
    ///
    /// Returns: The first stripped non-empty argument.
    /// Returns: An empty string, if there is no non-empty argument.
    async fn expand_statement_or(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs < 2 {
            return self.stmterr("OR: invalid args");
        }
        for arg in args {
            let trimmed = arg.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
        Ok(String::new())
    }

    /// Logically invert the boolean argument.
    ///
    /// Statement: $(not A)
    ///
    /// Returns: 1, if the stripped argument A is an empty string.
    /// Returns: An empty string otherwise.
    async fn expand_statement_not(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs != 1 {
            return self.stmterr("NOT: invalid args");
        }
        let result = if args[0].trim().is_empty() { "1" } else { "" };
        Ok(result.to_string())
    }

    /// Debug assertion.
    /// Aborts the program, if any argument is an empty string.
    ///
    /// Statement: $(assert A, ...)
    ///
    /// Returns an error, if any argument is empty after stripping.
    /// Returns: An empty string, otherwise.
    async fn expand_statement_assert(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs == 0 {
            return self.stmterr("ASSERT: missing argument");
        }
        let all_nonempty = args.iter().all(|a| !a.trim().is_empty());
        if !all_nonempty {
            return self.stmterr("ASSERT: failed");
        }
        Ok(String::new())
    }

    /// Strip whitespace at the start and at the end of all arguments.
    /// Concatenate all arguments.
    ///
    /// Statement: $(strip A, ...)
    ///
    /// Returns: All arguments stripped and concatenated.
    async fn expand_statement_strip(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        let mut result = String::with_capacity(if nargs > 0 { args[0].len() } else { 0 });
        for arg in args {
            result.push_str(arg.trim());
        }
        Ok(result)
    }

    /// Select an item from a list.
    /// Splits the STRING argument into tokens and return the N'th token.
    /// The token SEPARATOR defaults to whitespace.
    ///
    /// Statement: $(item STRING, N)
    /// Statement: $(item STRING, N, SEPARATOR)
    ///
    /// Returns: The N'th token.
    async fn expand_statement_item(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs != 2 && nargs != 3 {
            return self.stmterr("ITEM: invalid args");
        }
        let string = &args[0];
        let n = &args[1];
        let Ok(n) = parse_usize(n) else {
            return self.stmterr("ITEM: N is not an integer");
        };
        let sep = if nargs == 3 { args[2].trim() } else { "" };
        if sep.is_empty() {
            Ok(string
                .split_ascii_whitespace()
                .nth(n)
                .unwrap_or("")
                .to_string())
        } else {
            Ok(string.split(sep).nth(n).unwrap_or("").to_string())
        }
    }

    /// Check if a list contains an item.
    /// HAYSTACK is a list separated by SEPARATOR.
    /// SEPARATOR defaults to whitespace.
    ///
    /// Statement: $(contains HAYSTACK, NEEDLE)
    /// Statement: $(contains HAYSTACK, NEEDLE, SEPARATOR)
    ///
    /// Returns: NEEDLE, if HAYSTACK contains the stripped NEEDLE.
    async fn expand_statement_contains(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs != 2 && nargs != 3 {
            return self.stmterr("CONTAINS: invalid args");
        }
        let haystack = &args[0];
        let needle = args[1].trim();
        let sep = if nargs == 3 { args[2].trim() } else { "" };
        let result = if sep.is_empty() {
            haystack.split_ascii_whitespace().any(|x| x == needle)
        } else {
            haystack.split(sep).any(|x| x == needle)
        };
        Ok(if result {
            needle.to_string()
        } else {
            "".to_string()
        })
    }

    /// Cut a sub string out of the STRING argument.
    /// START is the first character index of the sub string.
    /// END is the last character index of the sub string plus 1.
    /// END defaults to START + 1.
    ///
    /// Statement: $(substr STRING, START)
    /// Statement: $(substr STRING, START, END)
    ///
    /// Returns: The sub string of STRING starting at START index up to (but not including) END index.
    async fn expand_statement_substr(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs != 2 && nargs != 3 {
            return self.stmterr("SUBSTR: invalid args");
        }
        let string: Vec<char> = args[0].chars().collect();
        let Ok(mut start) = parse_usize(&args[1]) else {
            return self.stmterr("SUBSTR: START is not a valid integer");
        };
        let mut end = if nargs == 3 {
            let Ok(end) = parse_usize(&args[2]) else {
                return self.stmterr("SUBSTR: END is not a valid integer");
            };
            end
        } else {
            string.len()
        };
        start = start.min(string.len());
        end = end.min(string.len());
        let substr = string[start..end].iter().collect();
        Ok(substr)
    }

    /// Sanitize a string.
    /// Concatenates all arguments with an underscore as separator.
    /// Replaces all non-alphanumeric characters by an underscore. Forces lower-case.
    ///
    /// Statement: $(sanitize STRING, ...)
    ///
    /// Returns: The sanitized string.
    async fn expand_statement_sanitize(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs == 0 {
            return self.stmterr("SANITIZE: invalid args");
        }
        let mut string = args.join("_");
        let mut cleaned = String::with_capacity(string.len());
        string.make_ascii_lowercase();
        let string = string.chars().map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        });
        let mut prev = None;
        for c in string {
            if c != '_' || Some(c) != prev {
                cleaned.push(c);
            }
            prev = Some(c);
        }
        Ok(cleaned)
    }

    async fn expand_statement_file_exists(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        //TODO
        Ok(String::new())
    }

    async fn expand_statement_file_mdatet(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        //TODO
        Ok(String::new())
    }

    /// Generate the site index.
    ///
    /// Statement: $(index)
    ///
    /// Returns: The site index.
    async fn expand_statement_index(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs != 1 || !args[0].trim().is_empty() {
            return self.stmterr("INDEX: invalid args");
        }
        self.index_refs.push(IndexRef::new(self.char_index));
        Ok(String::new())
    }

    async fn expand_statement_anchor(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        //TODO
        Ok(String::new())
    }

    async fn expand_statement_pagelist(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        //TODO
        Ok(String::new())
    }

    /// Generate a random number.
    /// BEGIN defaults to 0.
    /// END defaults to 65535.
    ///
    /// Statement: $(random)
    /// Statement: $(random BEGIN)
    /// Statement: $(random BEGIN, END)
    ///
    /// Returns: A random integer in the range from BEGIN to END (including both end points).
    async fn expand_statement_random(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs > 2 {
            return self.stmterr("RANDOM: invalid args");
        }
        let begin = if nargs >= 1 && !args[0].trim().is_empty() {
            let Ok(begin) = parse_i64(&args[0]) else {
                return self.stmterr("RANDOM: invalid BEGIN");
            };
            begin
        } else {
            0
        };
        let end = if nargs >= 2 && !args[1].trim().is_empty() {
            let Ok(end) = parse_i64(&args[1]) else {
                return self.stmterr("RANDOM: invalid END");
            };
            end
        } else {
            65535
        };
        let random: i64 = thread_rng().gen_range(begin..=end);
        Ok(format!("{random}"))
    }

    /// Select a random item.
    ///
    /// Statement: $(randitem ITEM0, ...)
    ///
    /// Returns: One random item of its arguments.
    async fn expand_statement_randitem(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let Some(item) = args.choose(&mut thread_rng()) else {
            return self.stmterr("RANDITEM: too few args");
        };
        Ok(item.to_string())
    }

    async fn expand_statement_arithmetic<F>(
        &mut self,
        chars: &mut Chars<'_>,
        op: &str,
        f: F,
    ) -> ah::Result<String>
    where
        F: FnOnce(f64, f64) -> f64,
    {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs != 2 {
            return self.stmterr(&format!("{op}: invalid args"));
        }
        let a = parse_f64(&args[0]).unwrap_or(0.0);
        let b = parse_f64(&args[1]).unwrap_or(0.0);
        let res = f(a, b);
        if res.is_finite() {
            let rounded = res.round();
            const EPSILON: f64 = 1e-6;
            if (res - rounded).abs() >= EPSILON
                || rounded < i64::MIN as f64
                || rounded > i64::MAX as f64
            {
                Ok(format!("{res}"))
            } else {
                let as_int = rounded as i64;
                Ok(format!("{as_int}"))
            }
        } else {
            self.stmterr(&format!("{op}: Arithmetic error: Result is {res}"))
        }
    }

    /// Add two numbers (integer or float).
    /// Returns the result as an integer, if it is representable as an integer.
    /// Otherwise returns the result as a floating point number.
    ///
    /// Statement: $(add A, B)
    ///
    /// Returns: The result of A + B
    async fn expand_statement_add(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        self.expand_statement_arithmetic(chars, "ADD", |a, b| a + b)
            .await
    }

    /// Subtract two numbers (integer or float).
    /// Returns the result as an integer, if it is representable as an integer.
    /// Otherwise returns the result as a floating point number.
    ///
    /// Statement: $(sub A, B)
    ///
    /// Returns: The result of A - B
    async fn expand_statement_sub(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        self.expand_statement_arithmetic(chars, "SUB", |a, b| a - b)
            .await
    }

    /// Multiply two numbers (integer or float).
    /// Returns the result as an integer, if it is representable as an integer.
    /// Otherwise returns the result as a floating point number.
    ///
    /// Statement: $(mul A, B)
    ///
    /// Returns: The result of A * B
    async fn expand_statement_mul(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        self.expand_statement_arithmetic(chars, "MUL", |a, b| a * b)
            .await
    }

    /// Divide two numbers (integer or float).
    /// Returns the result as an integer, if it is representable as an integer.
    /// Otherwise returns the result as a floating point number.
    ///
    /// Statement: $(div A, B)
    ///
    /// Returns: The result of A / B
    async fn expand_statement_div(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        self.expand_statement_arithmetic(chars, "DIV", |a, b| a / b)
            .await
    }

    /// Divide two numbers (integer or float) and get the remainder.
    /// Returns the result as an integer, if it is representable as an integer.
    /// Otherwise returns the result as a floating point number.
    ///
    /// Statement: $(mod A, B)
    ///
    /// Returns: The result of remainder(A / B)
    async fn expand_statement_mod(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        self.expand_statement_arithmetic(chars, "MOD", |a, b| a % b)
            .await
    }

    /// Round a floating point number to the next integer.
    /// If NDIGITS is specified, then round to this number of decimal digits.
    ///
    /// Statement: $(round A)
    /// Statement: $(round A, NDIGITS)
    ///
    /// Returns: Argument A rounded.
    async fn expand_statement_round(&mut self, chars: &mut Chars<'_>) -> ah::Result<String> {
        let args = self.parse_args(chars).await?;
        let nargs = args.len();
        if nargs != 1 && nargs != 2 {
            return self.stmterr("ROUND: invalid args");
        }
        let a = parse_f64(&args[0]).unwrap_or(0.0);
        let b = if nargs >= 2 {
            parse_i64(&args[1]).unwrap_or(0).clamp(0, 64) as usize
        } else {
            0
        };
        if b == 0 {
            let rounded = a.round().clamp(i64::MIN as f64, i64::MAX as f64) as i64;
            Ok(format!("{rounded}"))
        } else {
            Ok(format!("{:.1$}", a, b))
        }
    }

    #[rustfmt::skip]
    async fn expand_statement(
        &mut self,
        stmt_name: &str,
        chars: &mut Chars<'_>,
    ) -> ah::Result<String> {
        match stmt_name {
            // conditional / string compare / boolean
            "if" => self.expand_statement_if(chars).await,
            "eq" => self.expand_statement_eq(chars).await,
            "ne" => self.expand_statement_ne(chars).await,
            "and" => self.expand_statement_and(chars).await,
            "or" => self.expand_statement_or(chars).await,
            "not" => self.expand_statement_not(chars).await,

            // debugging
            "assert" => self.expand_statement_assert(chars).await,

            // string processing
            "strip" => self.expand_statement_strip(chars).await,
            "item" => self.expand_statement_item(chars).await,
            "contains" => self.expand_statement_contains(chars).await,
            "substr" => self.expand_statement_substr(chars).await,
            "sanitize" => self.expand_statement_sanitize(chars).await,

            // filesystem access
            "file_exists" => self.expand_statement_file_exists(chars).await,
            "file_mdatet" => self.expand_statement_file_mdatet(chars).await,

            // page index / page info
            "index" => self.expand_statement_index(chars).await,
            "anchor" => self.expand_statement_anchor(chars).await,
            "pagelist" => self.expand_statement_pagelist(chars).await,

            // random numbers
            "random" => self.expand_statement_random(chars).await,
            "randitem" => self.expand_statement_randitem(chars).await,

            // arithmetic
            "add" => self.expand_statement_add(chars).await,
            "sub" => self.expand_statement_sub(chars).await,
            "mul" => self.expand_statement_mul(chars).await,
            "div" => self.expand_statement_div(chars).await,
            "mod" => self.expand_statement_mod(chars).await,
            "round" => self.expand_statement_round(chars).await,

            _ => Ok(String::new()),//TODO error
        }
    }

    fn expand_variable(&self, var_name: &str) -> ah::Result<String> {
        Ok(self.vars.get(var_name))
    }

    fn skip_comment(&mut self, chars: &mut Chars<'_>) {
        let prev = chars.peek_bwd_nth(1).cloned();

        // Consume prefix.
        let _ = self.next(chars); // consume '!'
        let _ = self.next(chars); // consume '-'
        let _ = self.next(chars); // consume '-'
        let _ = self.next(chars); // consume '-'

        // Consume comment body.
        loop {
            let Some(c) = self.next(chars) else {
                break;
            };
            if c == '-'
                && chars.peek_nth(0) == Some(&'-')
                && chars.peek_nth(1) == Some(&'-')
                && chars.peek_nth(2) == Some(&'>')
            {
                // Consume suffix.
                let _ = self.next(chars); // consume '-'
                let _ = self.next(chars); // consume '-'
                let _ = self.next(chars); // consume '>'
                break;
            }
        }

        /* If the comment is on a line of its own, remove the line. */
        let next = chars.peek();
        if (prev.is_none() || prev == Some('\n')) && next == Some(&'\n') {
            let _ = self.next(chars); // consume '\n'
        }
    }

    async fn expand(&mut self, chars: &mut Chars<'_>, stop_chars: &[char]) -> ah::Result<String> {
        let mut exp = String::with_capacity(EXPAND_CAPACITY_DEF);
        'mainloop: while let Some(c) = self.next(chars) {
            let mut res: Option<String> = None;
            match c {
                '\\' if chars
                    .peek()
                    .map(|c| ESCAPE_CHARS.contains(c))
                    .unwrap_or(false) =>
                {
                    // Escaped characters
                    // Keep escapes. They are removed later.
                    let mut r = String::with_capacity(2);
                    r.push(c);
                    r.push(self.next(chars).unwrap());
                    res = Some(r);
                }
                '<' if chars.peek_nth(0) == Some(&'!')
                    && chars.peek_nth(1) == Some(&'-')
                    && chars.peek_nth(2) == Some(&'-')
                    && chars.peek_nth(3) == Some(&'-') =>
                {
                    // Comment
                    res = Some("".to_string()); // drop '<'
                    self.skip_comment(chars); // consume comment
                }
                _ if stop_chars.contains(&c) => {
                    // Stop character
                    break 'mainloop;
                }
                '@' => {
                    // Macro call
                    match iter_cons_until(chars, '(') {
                        Ok(macro_name) => {
                            let _ = self.next(chars); // consume '('
                            res = Some(self.do_macro(&macro_name, chars).await?);
                        }
                        Err(tail) => res = Some(tail),
                    }
                }
                '$' if chars.peek().map(|c| c.is_numeric()).unwrap_or(false) => {
                    // Macro argument
                    match iter_cons_until_not_in(chars, &NUMBER_CHARS) {
                        Ok(arg_name) => {
                            res = Some(self.expand_macro_arg(&arg_name)?);
                        }
                        Err(tail) => res = Some(tail),
                    }
                }
                '$' if chars.peek() == Some(&'(') => {
                    // Statement
                    match iter_cons_until_in(chars, &[' ', ')']) {
                        Ok(stmt_name) => {
                            let stmt_name = &stmt_name['('.len_utf8()..]; // Remove '('.
                            let _ = self.next(chars); // consume ' ' or ')'
                            res = Some(self.expand_statement(stmt_name, chars).await?);
                        }
                        Err(tail) => res = Some(tail),
                    }
                }
                '$' => {
                    // Variable
                    match iter_cons_until_not_in(chars, &VARNAME_CHARS) {
                        Ok(var_name) => {
                            res = Some(self.expand_variable(&var_name)?);
                        }
                        Err(tail) => res = Some(tail),
                    }
                }
                _ => (),
            }
            if let Some(res) = res {
                self.char_index += res.len();
                exp.push_str(&res);
            } else {
                self.char_index += c.len_utf8();
                exp.push(c);
            }
        }
        self.char_index -= exp.len();
        Ok(exp)
    }

    pub async fn run(mut self, input: &str) -> String {
        let mut chars = Chars::new(input.chars());
        let data = match self.expand(&mut chars, &[]).await {
            Ok(data) => data,
            Err(e) => {
                return format!("Resolver error: {e}"); //TODO return error?
            }
        };
        //TODO indices
        Self::unescape(&data)
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

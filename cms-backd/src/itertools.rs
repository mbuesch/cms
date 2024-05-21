// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub trait Char {
    fn get(&self) -> char;
}

impl Char for char {
    #[inline]
    fn get(&self) -> char {
        *self
    }
}

pub trait Peek {
    fn peek_next(&mut self) -> Option<&impl Char>;
    fn cons_next(&mut self) -> Option<impl Char>;
}

impl<I, const A: usize, const B: usize> Peek for peekable_fwd_bwd::Peekable<I, A, B>
where
    I: Iterator,
    I::Item: Char,
    I::Item: Clone,
{
    #[inline]
    fn peek_next(&mut self) -> Option<&impl Char> {
        self.peek()
    }

    #[inline]
    fn cons_next(&mut self) -> Option<impl Char> {
        self.next()
    }
}

impl<I> Peek for std::iter::Peekable<I>
where
    I: Iterator,
    I::Item: Char,
{
    #[inline]
    fn peek_next(&mut self) -> Option<&impl Char> {
        self.peek()
    }

    #[inline]
    fn cons_next(&mut self) -> Option<impl Char> {
        self.next()
    }
}

#[inline]
fn iter_cons_until_generic<P: Peek>(
    iter: &mut P,
    chars: &[char],
    invert: bool,
) -> Result<String, String> {
    let mut ret = String::with_capacity(64);
    while let Some(c) = iter.peek_next() {
        let c = c.get();
        if chars.contains(&c) ^ invert {
            return Ok(ret);
        }
        iter.cons_next(); // consume char.
        ret.push(c);
    }
    Err(ret)
}

pub fn iter_cons_until_not_in<P: Peek>(iter: &mut P, chars: &[char]) -> Result<String, String> {
    iter_cons_until_generic(iter, chars, true)
}

pub fn iter_cons_until_in<P: Peek>(iter: &mut P, chars: &[char]) -> Result<String, String> {
    iter_cons_until_generic(iter, chars, false)
}

pub fn iter_cons_until<P: Peek>(iter: &mut P, ch: char) -> Result<String, String> {
    iter_cons_until_generic(iter, &[ch], false)
}

#[cfg(test)]
mod tests {
    use super::*;
    type Peekable<'a> = peekable_fwd_bwd::Peekable<std::str::Chars<'a>, 1, 8>;

    #[test]
    fn test_iter_cons_until() {
        let mut it = Peekable::new("abc(def".chars());
        let a = iter_cons_until(&mut it, '(');
        assert_eq!(a, Ok("abc".to_string()));
        assert_eq!(it.next(), Some('('));

        let mut it = Peekable::new("abcdef".chars());
        let a = iter_cons_until(&mut it, '(');
        assert_eq!(a, Err("abcdef".to_string()));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_iter_cons_until_in() {
        let mut it = Peekable::new("abc()def".chars());
        let a = iter_cons_until_in(&mut it, &['(', ')']);
        assert_eq!(a, Ok("abc".to_string()));
        assert_eq!(it.next(), Some('('));
        let a = iter_cons_until_in(&mut it, &['(', ')']);
        assert_eq!(a, Ok("".to_string()));
        assert_eq!(it.next(), Some(')'));

        let mut it = Peekable::new("abcdef".chars());
        let a = iter_cons_until_in(&mut it, &['(', ')']);
        assert_eq!(a, Err("abcdef".to_string()));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_iter_cons_until_not_in() {
        let mut it = Peekable::new("abc(def".chars());
        let a = iter_cons_until_not_in(&mut it, &['a', 'b', 'c', 'd', 'e', 'f']);
        assert_eq!(a, Ok("abc".to_string()));
        assert_eq!(it.next(), Some('('));

        let mut it = Peekable::new("abcdef".chars());
        let a = iter_cons_until_not_in(&mut it, &['a', 'b', 'c', 'd', 'e', 'f']);
        assert_eq!(a, Err("abcdef".to_string()));
        assert_eq!(it.next(), None);
    }
}

// vim: ts=4 sw=4 expandtab

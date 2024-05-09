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

use multipeek::MultiPeek;
use std::iter::Peekable;

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

impl<I> Peek for MultiPeek<I>
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

impl<I> Peek for Peekable<I>
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
    use multipeek::IteratorExt as _;

    #[test]
    fn test_iter_cons_until() {
        let mut it = "abc(def".chars().multipeek();
        let a = iter_cons_until(&mut it, '(');
        assert_eq!(a, Ok("abc".to_string()));
        assert_eq!(it.next(), Some('('));

        let mut it = "abcdef".chars().multipeek();
        let a = iter_cons_until(&mut it, '(');
        assert_eq!(a, Err("abcdef".to_string()));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_iter_cons_until_in() {
        let mut it = "abc()def".chars().multipeek();
        let a = iter_cons_until_in(&mut it, &['(', ')']);
        assert_eq!(a, Ok("abc".to_string()));
        assert_eq!(it.next(), Some('('));
        let a = iter_cons_until_in(&mut it, &['(', ')']);
        assert_eq!(a, Ok("".to_string()));
        assert_eq!(it.next(), Some(')'));

        let mut it = "abcdef".chars().multipeek();
        let a = iter_cons_until_in(&mut it, &['(', ')']);
        assert_eq!(a, Err("abcdef".to_string()));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_iter_cons_until_not_in() {
        let mut it = "abc(def".chars().multipeek();
        let a = iter_cons_until_not_in(&mut it, &['a', 'b', 'c', 'd', 'e', 'f']);
        assert_eq!(a, Ok("abc".to_string()));
        assert_eq!(it.next(), Some('('));

        let mut it = "abcdef".chars().multipeek();
        let a = iter_cons_until_not_in(&mut it, &['a', 'b', 'c', 'd', 'e', 'f']);
        assert_eq!(a, Err("abcdef".to_string()));
        assert_eq!(it.next(), None);
    }
}

// vim: ts=4 sw=4 expandtab

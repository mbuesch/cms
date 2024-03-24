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

#![forbid(unsafe_code)]

use anyhow as ah;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    ops::Deref,
    path::{Path, PathBuf},
    str::{FromStr, Split},
};

const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
const NUMBERS: &str = "0123456789";
const ELEMEXTRA: &str = "-_.";

const ELEMSEP: char = '/';

const MAX_IDENTSTR_LEN: usize = 512;
const MAX_IDENT_DEPTH: usize = 32;

/// Element format (for checking).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum ElemFmt {
    /// "User element". Must not start with double-underscore.
    User,
    /// "System element". May start with double-underscore.
    System,
}

/// Check if the identifier path element string contains an invalid character.
#[inline]
fn check_ident_elem(elem: &str, fmt: ElemFmt) -> ah::Result<()> {
    #[inline]
    fn is_valid_ident_char(c: char) -> bool {
        UPPERCASE.contains(c)
            || LOWERCASE.contains(c)
            || NUMBERS.contains(c)
            || ELEMEXTRA.contains(c)
    }

    if elem.starts_with('.') {
        // No ".", ".." and hidden files.
        return Err(ah::format_err!("Invalid identifier: Starts with dot."));
    }
    if fmt != ElemFmt::System && elem.starts_with("__") {
        // System files/dirs (starting with "__") not allowed.
        return Err(ah::format_err!("Invalid identifier: 'Dunder' not allowed."));
    }
    if !elem.chars().all(is_valid_ident_char) {
        return Err(ah::format_err!("Invalid identifier: Invalid character."));
    }
    Ok(())
}

/// An unchecked identifier path.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ident(String);

impl FromStr for Ident {
    type Err = Infallible;

    /// Create a new identifier from an untrusted string.
    #[inline]
    fn from_str(ident: &str) -> Result<Ident, Infallible> {
        Ok(Ident(ident.to_string()))
    }
}

impl Ident {
    /// Returns a reference to the raw string.
    #[inline]
    fn as_str(&self) -> &str {
        &self.0
    }

    /// Get an iterator over all ident path elements.
    ///
    /// Note that this returns an iterator that yields one empty element
    /// for the special case of the empty identifier path `""`.
    #[inline]
    fn elements(&self) -> Split<'_, char> {
        self.0.split(ELEMSEP)
    }

    /// Get the last path element as a &str.
    ///
    /// Returns None, if this identifier has zero elements.
    #[inline]
    pub fn last_element_str(&self) -> Option<&str> {
        if self.0.is_empty() {
            None
        } else {
            self.elements().last()
        }
    }

    /// Get the number of path elements.
    #[inline]
    fn element_count(&self) -> usize {
        if self.0.is_empty() {
            0
        } else {
            self.elements().count()
        }
    }

    /// Clone self and append one element.
    pub fn clone_append(&self, append_elem: &str) -> Ident {
        let mut new = self.clone();
        new.0.push(ELEMSEP);
        new.0.push_str(append_elem);
        new
    }

    /// Clean up the identifier.
    /// The result is still unchecked and untrusted.
    pub fn into_cleaned_path(mut self) -> Ident {
        let mut s = self.0;

        // Strip leading and trailing whitespace and slashes.
        let trimmed = s.trim_matches(&[' ', '\t', '/']);
        if s != trimmed {
            s = trimmed.to_string();
        }

        // Special case: Index is the root page.
        if ["index.html", "index.php"].contains(&s.as_str()) {
            s.clear();
        }

        // Remove virtual page file extensions.
        if s.ends_with(".html") || s.ends_with(".php") {
            s.drain(s.rfind('.').unwrap()..);
        }

        self.0 = s;
        self
    }

    /// Check if the identifier ends with the specified [tail] str.
    #[inline]
    pub fn ends_with(&self, tail: &str) -> bool {
        self.as_str().ends_with(tail)
    }

    fn check(&self, max_ident_depth: usize, elem_fmt: ElemFmt) -> ah::Result<()> {
        // Check string size limit.
        if self.0.len() > MAX_IDENTSTR_LEN {
            return Err(ah::format_err!("Invalid identifier: String too long."));
        }

        // Check if each ident path element contains only valid characters.
        for (i, elem) in self.elements().enumerate() {
            // Path depth too deep?
            if i >= max_ident_depth {
                return Err(ah::format_err!("Invalid identifier: Ident path too deep."));
            }
            // Path element contains invalid characters?
            check_ident_elem(elem, elem_fmt)?;
        }

        Ok(())
    }

    /// Convert this [Ident] into a trusted [CheckedIdent].
    #[inline]
    pub fn into_checked(self) -> ah::Result<CheckedIdent> {
        // Run the checks.
        self.check(MAX_IDENT_DEPTH, ElemFmt::User)?;
        // The ident is safe. Seal it in a read-only wrapper.
        Ok(CheckedIdent(self))
    }

    /// Convert this [Ident] into a trusted [CheckedIdentElem].
    #[inline]
    pub fn into_checked_element(self) -> ah::Result<CheckedIdentElem> {
        // Run the checks.
        // check(1): Only one element deep.
        self.check(1, ElemFmt::User)?;
        // The ident is safe. Seal it in a read-only wrapper.
        Ok(CheckedIdentElem(self))
    }

    /// Convert this [Ident] into a trusted [CheckedIdentElem] with system name.
    #[inline]
    pub fn into_checked_sys_element(self) -> ah::Result<CheckedIdentElem> {
        // Run the checks.
        // check(1): Only one element deep.
        self.check(1, ElemFmt::System)?;
        // The ident is safe. Seal it in a read-only wrapper.
        Ok(CheckedIdentElem(self))
    }
}

/// A checked wrapper around [Ident].
///
/// The [Ident] path contained herein has been checked and found to
/// contain no fs-unsafe characters.
///
/// This can only be constructed via [Ident::into_checked]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CheckedIdent(Ident);

/// A checked wrapper around [Ident].
///
/// The [Ident] path contained herein has been checked and found to
/// contain no fs-unsafe characters.
/// It is also guaranteed to only contain one single path element (no slash).
///
/// This can only be constructed via [Ident::into_checked_element]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CheckedIdentElem(Ident);

/// Number of elements to strip off the path elements.
pub enum Strip {
    /// Do not strip.
    No,
    /// Strip off this many elements from the right hand side.
    Right(usize),
}

/// Optional tail element to add to the filesystem path.
pub enum Tail {
    /// No tail.
    None,
    /// One tail element.
    One(CheckedIdentElem),
    /// Two tail elements.
    Two(CheckedIdentElem, CheckedIdentElem),
}

macro_rules! impl_checked_ident {
    ($name:ident) => {
        impl $name {
            /// Downgrade to an unchecked [Ident].
            #[inline]
            pub fn downgrade(self) -> Ident {
                self.0
            }

            /// Get a shared reference to this as an unchecked [Ident].
            #[inline]
            pub fn as_downgrade_ref(&self) -> &Ident {
                &self.0
            }

            /// Convert this checked identifier into a safe [PathBuf]
            /// for filesystem use.
            ///
            /// The [base] is the start of the filesystem path.
            ///
            /// The elements of the identifier are added between [base] and [tail].
            ///
            /// The [tail] is optionally added to the end of the path.
            ///
            /// Warning: [base] is not checked for safe filesystem access.
            #[inline]
            pub fn to_fs_path(&self, base: &Path, tail: &Tail) -> PathBuf {
                self.to_stripped_fs_path(base, Strip::No, tail).unwrap()
            }

            /// Same as [to_fs_path], but may strip some elements
            /// from the identifier elements.
            #[inline]
            pub fn to_stripped_fs_path(
                &self,
                base: &Path,
                strip: Strip,
                tail: &Tail,
            ) -> ah::Result<PathBuf> {
                let mut path = PathBuf::with_capacity(1024);

                // Add base path.
                path.push(base);

                // Add all path elements.
                let elem_count = match strip {
                    Strip::No => usize::MAX,
                    Strip::Right(n) => {
                        let count = self.element_count();
                        if n > count {
                            return Err(ah::format_err!("Fs path stripping underflow."));
                        }
                        count.saturating_sub(n)
                    }
                };
                for elem in self.elements().take(elem_count) {
                    path.push(elem);
                }

                // Add the tail, if any.
                match tail {
                    Tail::None => (),
                    Tail::One(tail) => {
                        path.push(tail.as_str());
                    }
                    Tail::Two(first, second) => {
                        path.push(first.as_str());
                        path.push(second.as_str());
                    }
                }

                Ok(path)
            }
        }

        impl Deref for $name {
            type Target = Ident;

            #[inline]
            fn deref(&self) -> &Self::Target {
                self.as_downgrade_ref()
            }
        }
    };
}

impl_checked_ident!(CheckedIdent);
impl_checked_ident!(CheckedIdentElem);

// vim: ts=4 sw=4 expandtab

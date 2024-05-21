// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

use anyhow::{self as ah, format_err as err};
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
        return Err(err!("Invalid identifier: Starts with dot."));
    }
    if fmt != ElemFmt::System && elem.starts_with("__") {
        // System files/dirs (starting with "__") not allowed.
        return Err(err!("Invalid identifier: 'Dunder' not allowed."));
    }
    if !elem.chars().all(is_valid_ident_char) {
        return Err(err!("Invalid identifier: Invalid character."));
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

impl Default for Ident {
    #[inline]
    fn default() -> Self {
        Self::ROOT.clone()
    }
}

impl Ident {
    /// Ident path of the root `/`.
    pub const ROOT: Ident = Ident(String::new());

    /// Returns a reference to the raw string.
    #[inline]
    fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this ident path is the root ident.
    #[inline]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// Check if this ident path starts with all elements from another ident path.
    #[inline]
    pub fn starts_with(&self, other: &Ident) -> bool {
        if other.0.is_empty() {
            false
        } else {
            self.0.starts_with(&other.0)
        }
    }

    /// Get an iterator over all ident path elements.
    ///
    /// Note that this returns an iterator that yields one empty element
    /// for the special case of the empty identifier path `""`.
    #[inline]
    fn elements(&self) -> Split<'_, char> {
        self.0.split(ELEMSEP)
    }

    /// Get the first path element as a &str.
    ///
    /// Returns None, if this identifier has zero elements.
    #[inline]
    pub fn first_element_str(&self) -> Option<&str> {
        if self.0.is_empty() {
            None
        } else {
            self.elements().next()
        }
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

    /// Get the n'th path element as a &str.
    ///
    /// Returns None, if this identifier has zero elements.
    #[inline]
    pub fn nth_element_str(&self, n: usize) -> Option<&str> {
        if self.0.is_empty() {
            None
        } else {
            self.elements().nth(n)
        }
    }

    /// Get the n'th path element as an Indent.
    ///
    /// Returns None, if this identifier has zero elements.
    #[inline]
    pub fn nth_element(&self, n: usize) -> Option<Self> {
        if let Some(s) = self.nth_element_str(n) {
            s.parse::<Ident>().ok()
        } else {
            None
        }
    }

    /// Get the number of path elements.
    #[inline]
    pub fn element_count(&self) -> usize {
        if self.0.is_empty() {
            0
        } else {
            self.elements().count()
        }
    }

    /// Clone self and append one element.
    pub fn clone_append(&self, append_elem: &str) -> Ident {
        assert!(!append_elem.contains(ELEMSEP));
        let mut new = self.clone();
        if !new.0.is_empty() {
            new.0.push(ELEMSEP);
        }
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

    /// Check if the identifier ends with the specified `tail` str.
    #[inline]
    pub fn ends_with(&self, tail: &str) -> bool {
        self.as_str().ends_with(tail)
    }

    fn check(&self, max_ident_depth: usize, elem_fmt: ElemFmt) -> ah::Result<()> {
        // Check string size limit.
        if self.0.len() > MAX_IDENTSTR_LEN {
            return Err(err!("Invalid identifier: String too long."));
        }

        // Check if each ident path element contains only valid characters.
        for (i, elem) in self.elements().enumerate() {
            // Path depth too deep?
            if i >= max_ident_depth {
                return Err(err!("Invalid identifier: Ident path too deep."));
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

    /// Convert this [Ident] into a trusted [CheckedIdent] with system name.
    #[inline]
    pub fn into_checked_sys(self) -> ah::Result<CheckedIdent> {
        // Run the checks.
        self.check(MAX_IDENT_DEPTH, ElemFmt::System)?;
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

impl CheckedIdent {
    /// Ident path of the root `/`.
    pub const ROOT: CheckedIdent = CheckedIdent(Ident::ROOT);
}

impl Default for CheckedIdent {
    #[inline]
    fn default() -> Self {
        Self::ROOT.clone()
    }
}

/// A checked wrapper around [Ident].
///
/// The [Ident] path contained herein has been checked and found to
/// contain no fs-unsafe characters.
/// It is also guaranteed to only contain one single path element (no slash).
///
/// This can only be constructed via [Ident::into_checked_element]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CheckedIdentElem(Ident);

impl Default for CheckedIdentElem {
    #[inline]
    fn default() -> Self {
        Self(Ident(String::new()))
    }
}

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

macro_rules! impl_common_checked_ident {
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

            /// Downgrade to an unchecked cloned [Ident].
            #[inline]
            pub fn downgrade_clone(&self) -> Ident {
                self.as_downgrade_ref().clone()
            }

            /// Convert this checked identifier into a safe [PathBuf]
            /// for filesystem use.
            ///
            /// The `base` is the start of the filesystem path.
            ///
            /// The elements of the identifier are added between `base` and `tail`.
            ///
            /// The `tail` is optionally added to the end of the path.
            ///
            /// Warning: `base` is not checked for safe filesystem access.
            #[inline]
            pub fn to_fs_path(&self, base: &Path, tail: &Tail) -> PathBuf {
                self.to_stripped_fs_path(base, Strip::No, tail).unwrap()
            }

            /// Same as `to_fs_path`, but may strip some elements
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
                            return Err(err!("Fs path stripping underflow."));
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

impl_common_checked_ident!(CheckedIdent);
impl_common_checked_ident!(CheckedIdentElem);

pub struct UrlComp<'a> {
    pub protocol: Option<&'a str>,
    pub domain: Option<&'a str>,
    pub base: Option<&'a str>,
}

impl CheckedIdent {
    /// Convert this [CheckedIdent] into an URL string.
    pub fn url(&self, comp: UrlComp<'_>) -> String {
        let mut url = String::with_capacity(128);

        if let Some(protocol) = &comp.protocol {
            url.push_str(protocol);
            url.push_str("://");
        }

        if let Some(domain) = &comp.domain {
            url.push_str(domain.trim_matches('/'));
            url.push('/');
        }

        if let Some(base) = &comp.base {
            if url.is_empty() {
                url.push('/');
            }
            url.push_str(base.trim_matches('/'));
            url.push('/');
        }

        if !self.as_str().is_empty() {
            if url.is_empty() {
                url.push('/');
            }
            for (i, elem) in self.elements().enumerate() {
                if i != 0 {
                    url.push('/');
                }
                url.push_str(elem);
            }
        }

        if !url.is_empty() && !url.ends_with('/') {
            url.push_str(".html");
        }

        url
    }
}

// vim: ts=4 sw=4 expandtab

// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(test)]
#![allow(non_camel_case_types)]

#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate test;

use libc::{c_uchar, c_int, c_void};

type re2_regexp = c_void;

#[repr(C)]
struct re2_string {
    text: *const c_uchar,
    len: c_int,
}

impl<'a> From<&'a str> for re2_string {
    fn from(s: &'a str) -> re2_string {
        re2_string { text: s.as_ptr(), len: s.len() as c_int }
    }
}

extern {
    fn re2_regexp_new(pat: re2_string) -> *mut re2_regexp;
    fn re2_regexp_free(re: *mut re2_regexp);
    fn re2_regexp_match(
        re: *mut re2_regexp,
        text: re2_string,
        startpos: c_int,
        endpos: c_int,
    ) -> bool;
}

struct Regex {
    re: *mut re2_regexp,
}

#[derive(Debug)]
struct Error(());

// Regex can't be used safely from multiple threads simultaneously, so this is
// a lie and therefore unsafe. It is, however, convenient and fine for the
// purposes of benchmarking where a Regex is only ever used in one thread.
unsafe impl Send for Regex {}
unsafe impl Sync for Regex {}

impl Drop for Regex {
    fn drop(&mut self) {
        unsafe { re2_regexp_free(self.re); }
    }
}

impl Regex {
    fn new(pattern: &str) -> Result<Regex, Error> {
        unsafe { Ok(Regex { re: re2_regexp_new(pattern.into()) }) }
    }

    fn is_match(&self, text: &str) -> bool {
        unsafe {
            re2_regexp_match(self.re, text.into(), 0, text.len() as c_int)
        }
    }
}

macro_rules! regex(
    ($re:expr) => { ::Regex::new($re).unwrap() }
);

mod misc;

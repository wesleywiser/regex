// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use set_exec::{SetExec, SetExecBuilder};

use Error;

pub struct RegexSetBuilder {
    res: Vec<String>,
}

impl RegexSetBuilder {
    pub fn new(re: &str) -> Self {
        RegexSetBuilder { res: vec![re.to_owned()] }
    }

    pub fn union(mut self, re: &str) -> Self {
        self.res.push(re.to_owned());
        self
    }

    pub fn compile(self) -> Result<RegexSet, Error> {
        SetExecBuilder::new(self.res).build().map(RegexSet)
    }
}

pub struct RegexSet(SetExec);

impl RegexSet {
    pub fn is_match(&self, text: &str) -> bool {
        let mut caps = self.0.alloc_captures();
        let m = self.0.exec(&mut caps, text, 0);
        println!("CAPS: {:?}", caps);
        m
    }
}

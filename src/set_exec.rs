// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use dfa;
use program::{Program, ProgramBuilder};
use Error;

pub struct SetExec {
    prog: Program,
    dfa: Program,
    dfa_reverse: Program,
    can_dfa: bool,
}

pub struct SetExecBuilder {
    res: Vec<String>,
    size_limit: usize,
}

impl SetExecBuilder {
    pub fn new(res: Vec<String>) -> Self {
        SetExecBuilder {
            res: res,
            size_limit: 10 * (1 << 20),
        }
    }

    pub fn size_limit(mut self, bytes: usize) -> Self {
        self.size_limit = bytes;
        self
    }

    pub fn build(self) -> Result<SetExec, Error> {
        let prog = try!(
            ProgramBuilder::new_many(&self.res)
                           .size_limit(self.size_limit)
                           .compile());
        let mut dfa = try!(
            ProgramBuilder::new_many(&self.res)
                           .size_limit(self.size_limit)
                           .dfa(true)
                           .compile());
        // Because the literal finder on byte-based programs is sub-optimal.
        // We can use the literals found from a Unicode-based program just
        // fine for now.
        dfa.prefixes = prog.prefixes.clone();
        let dfa_reverse = try!(
            ProgramBuilder::new_many(&self.res)
                           .size_limit(self.size_limit)
                           .dfa(true)
                           .reverse(true)
                           .compile());
        let can_dfa = dfa::can_exec(&dfa.insts);
        Ok(SetExec {
            prog: prog,
            dfa: dfa,
            dfa_reverse: dfa_reverse,
            can_dfa: can_dfa,
        })
    }
}

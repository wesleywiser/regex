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
use exec::Search;
use input::{ByteInput, CharInput};
use nfa::Nfa;
use program::{Program, ProgramBuilder};
use re::CaptureIdxs;

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
        // let can_dfa = dfa::can_exec(&dfa.insts);
        Ok(SetExec {
            prog: prog,
            dfa: dfa,
            dfa_reverse: dfa_reverse,
            can_dfa: false,
        })
    }
}

impl SetExec {
    pub fn exec(
        &self,
        caps: &mut CaptureIdxs,
        text: &str,
        start: usize,
    ) -> bool {
        if self.can_dfa {
            self.exec_dfa(caps, text, start)
        } else {
            self.exec_nfa(caps, text, start)
        }
    }

    fn exec_nfa(
        &self,
        caps: &mut CaptureIdxs,
        text: &str,
        start: usize,
    ) -> bool {
        let mut matches = vec![false; self.prog.insts.matches().len()];
        let m = if self.prog.insts.is_bytes() {
            Nfa::exec(&self.prog, ByteInput::new(text), start, Search {
                caps: caps,
                matches: &mut matches,
            })
        } else {
            Nfa::exec(&self.prog, CharInput::new(text), start, Search {
                caps: caps,
                matches: &mut matches,
            })
        };
        println!("MATCHES: {:?}", matches);
        m
    }

    fn exec_dfa(
        &self,
        caps: &mut CaptureIdxs,
        text: &str,
        start: usize,
    ) -> bool {
        unreachable!()
    }

    /// Return a fresh allocation for storing all possible captures in the
    /// underlying regular expression.
    pub fn alloc_captures(&self) -> Vec<Option<usize>> {
        self.prog.alloc_captures()
    }
}

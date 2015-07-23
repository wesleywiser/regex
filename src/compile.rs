// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use syntax::{Expr, Repeater, CharClass, ClassRange};

use Error;
use program::{CharRanges, Inst, InstIdx};

type Compiled = (Vec<Inst>, Vec<Option<String>>);

/// A regex compiler.
///
/// A regex compiler is responsible for turning a regex's AST into a sequence
/// of instructions.
pub struct Compiler {
    size_limit: usize,
    insts: Vec<Inst>,
    cap_names: Vec<Option<String>>,
    reverse: bool,
}

impl Compiler {
    /// Creates a new compiler that limits the size of the regex program
    /// to the size given (in bytes).
    pub fn new(size_limit: usize) -> Compiler {
        Compiler {
            size_limit: size_limit,
            insts: vec![],
            cap_names: vec![None],
            reverse: false,
        }
    }

    /// Compiles the given regex AST into a tuple of a sequence of
    /// instructions and a sequence of capture groups, optionally named.
    pub fn compile(mut self, ast: &Expr) -> Result<Compiled, Error> {
        self.insts.push(Inst::Save(0));
        try!(self.c(ast));
        self.insts.push(Inst::Save(1));
        self.insts.push(Inst::Match);
        Ok((self.insts, self.cap_names))
    }

    /// Compiles the given regex AST into a tuple of a sequence of
    /// instructions and a sequence of capture groups, optionally named.
    pub fn compile_reverse(mut self, ast: &Expr) -> Result<Vec<Inst>, Error> {
        self.reverse = true;
        self.insts.push(Inst::Save(0));
        try!(self.c(ast));
        self.insts.push(Inst::Save(1));
        self.insts.push(Inst::Match);
        Ok(self.insts)
    }

    fn c(&mut self, ast: &Expr) -> Result<(), Error> {
        use program::Inst::*;
        use program::LookInst::*;

        match *ast {
            Expr::Empty => {},
            Expr::Literal { ref chars, casei } => {
                let it: Box<Iterator<Item=char>> = if self.reverse {
                    Box::new(chars.iter().cloned().rev())
                } else {
                    Box::new(chars.iter().cloned())
                };
                for c in it {
                    if casei {
                        try!(self.c(&Expr::Class(CharClass::new(vec![
                            ClassRange { start: c, end: c },
                        ]).case_fold())));
                    } else {
                        self.push(Char(c));
                    }
                }
            }
            Expr::AnyChar => self.push(Ranges(CharRanges::any())),
            Expr::AnyCharNoNL => self.push(Ranges(CharRanges::any_nonl())),
            Expr::Class(ref cls) => {
                if cls.len() == 1 && cls[0].start == cls[0].end {
                    self.push(Char(cls[0].start));
                } else {
                    self.push(Ranges(CharRanges::from_class(cls)));
                }
            }
            Expr::StartLine => self.push(EmptyLook(StartLine)),
            Expr::EndLine => self.push(EmptyLook(EndLine)),
            Expr::StartText => self.push(EmptyLook(StartText)),
            Expr::EndText => self.push(EmptyLook(EndText)),
            Expr::WordBoundary => self.push(EmptyLook(WordBoundary)),
            Expr::NotWordBoundary => self.push(EmptyLook(NotWordBoundary)),
            Expr::Group { ref e, i: None, name: None } => try!(self.c(e)),
            Expr::Group { ref e, i, ref name } => {
                let i = i.expect("capture index");
                if self.reverse {
                    self.cap_names.push(name.clone());
                    self.push(Save(2 * i));
                    try!(self.c(e));
                    self.push(Save(2 * i + 1));
                } else {
                    self.cap_names.push(name.clone());
                    self.push(Save(2 * i));
                    try!(self.c(e));
                    self.push(Save(2 * i + 1));
                }
            }
            Expr::Concat(ref es) => {
                if self.reverse {
                    for e in es.iter().rev() {
                        try!(self.c(e));
                    }
                } else {
                    for e in es {
                        try!(self.c(e));
                    }
                }
            }
            Expr::Alternate(ref es) => {
                if es.len() == 0 {
                    return Ok(());
                }
                let mut jmps_to_end = vec![];
                for e in &es[0..es.len()-1] {
                    let split = self.empty_split();
                    let j1 = self.insts.len();
                    try!(self.c(e));
                    jmps_to_end.push(self.empty_jump());
                    let j2 = self.insts.len();
                    self.set_split(split, j1, j2);
                }
                try!(self.c(&es[es.len()-1]));
                let end = self.insts.len();
                for jmp_to_end in jmps_to_end {
                    self.set_jump(jmp_to_end, end);
                }
            }
            Expr::Repeat { ref e, r: Repeater::ZeroOrOne, greedy } => {
                let split = self.empty_split();
                let j1 = self.insts.len();
                try!(self.c(e));
                let j2 = self.insts.len();

                if greedy || self.reverse {
                    self.set_split(split, j1, j2);
                } else {
                    self.set_split(split, j2, j1);
                }
            }
            Expr::Repeat { ref e, r: Repeater::ZeroOrMore, greedy } => {
                let j1 = self.insts.len();
                let split = self.empty_split();
                let j2 = self.insts.len();
                try!(self.c(e));
                let jmp = self.empty_jump();
                let j3 = self.insts.len();

                self.set_jump(jmp, j1);
                if greedy || self.reverse {
                    self.set_split(split, j2, j3);
                } else {
                    self.set_split(split, j3, j2);
                }
            }
            Expr::Repeat { ref e, r: Repeater::OneOrMore, greedy } => {
                let j1 = self.insts.len();
                try!(self.c(e));
                let split = self.empty_split();
                let j2 = self.insts.len();

                if greedy || self.reverse {
                    self.set_split(split, j1, j2);
                } else {
                    self.set_split(split, j2, j1);
                }
            }
            Expr::Repeat {
                ref e,
                r: Repeater::Range { min, max: None },
                greedy,
            } => {
                for _ in 0..min {
                    try!(self.c(e));
                }
                try!(self.c(&Expr::Repeat {
                    e: e.clone(),
                    r: Repeater::ZeroOrMore,
                    greedy: greedy,
                }));
            }
            Expr::Repeat {
                ref e,
                r: Repeater::Range { min, max: Some(max) },
                greedy,
            } => {
                for _ in 0..min {
                    try!(self.c(e));
                }
                // It is much simpler to compile, e.g., `a{2,5}` as:
                //
                //     aaa?a?a?
                //
                // But you end up with a sequence of instructions like this:
                //
                //     0: 'a'
                //     1: 'a',
                //     2: split(3, 4)
                //     3: 'a'
                //     4: split(5, 6)
                //     5: 'a'
                //     6: split(7, 8)
                //     7: 'a'
                //     8: MATCH
                //
                // This is *incredibly* inefficient because the splits end
                // up forming a chain. Given a much larger number than `5`,
                // it is easy cause perverse behavior in the matching engines
                // like stack overflows. We could fix the matching engine,
                // but instead, we should just make the program smarter.
                // Thus, we do a custom job here and instead of chaining the
                // splits together, we simply point them to the MATCH
                // instruction directly.
                let (mut splits, mut starts) = (vec![], vec![]);
                for _ in min..max {
                    splits.push(self.empty_split());
                    starts.push(self.insts.len());
                    try!(self.c(e));
                }
                let end = self.insts.len();
                for (split, start) in splits.into_iter().zip(starts) {
                    if greedy || self.reverse {
                        self.set_split(split, start, end);
                    } else {
                        self.set_split(split, end, start);
                    }
                }
            }
        }
        self.check_size()
    }

    fn check_size(&self) -> Result<(), Error> {
        use std::mem::size_of;

        if self.insts.len() * size_of::<Inst>() > self.size_limit {
            Err(Error::CompiledTooBig(self.size_limit))
        } else {
            Ok(())
        }
    }

    /// Appends the given instruction to the program.
    #[inline]
    fn push(&mut self, x: Inst) {
        self.insts.push(x)
    }

    /// Appends an *empty* `Split` instruction to the program and returns
    /// the index of that instruction. (The index can then be used to "patch"
    /// the actual locations of the split in later.)
    #[inline]
    fn empty_split(&mut self) -> InstIdx {
        self.insts.push(Inst::Split(0, 0));
        self.insts.len() - 1
    }

    /// Sets the left and right locations of a `Split` instruction at index
    /// `i` to `pc1` and `pc2`, respectively.
    /// If the instruction at index `i` isn't a `Split` instruction, then
    /// `panic!` is called.
    #[inline]
    fn set_split(&mut self, i: InstIdx, pc1: InstIdx, pc2: InstIdx) {
        let split = &mut self.insts[i];
        match *split {
            Inst::Split(_, _) => *split = Inst::Split(pc1, pc2),
            _ => panic!("BUG: Invalid split index."),
        }
    }

    /// Appends an *empty* `Jump` instruction to the program and returns the
    /// index of that instruction.
    #[inline]
    fn empty_jump(&mut self) -> InstIdx {
        self.insts.push(Inst::Jump(0));
        self.insts.len() - 1
    }

    /// Sets the location of a `Jump` instruction at index `i` to `pc`.
    /// If the instruction at index `i` isn't a `Jump` instruction, then
    /// `panic!` is called.
    #[inline]
    fn set_jump(&mut self, i: InstIdx, pc: InstIdx) {
        let jmp = &mut self.insts[i];
        match *jmp {
            Inst::Jump(_) => *jmp = Inst::Jump(pc),
            _ => panic!("BUG: Invalid jump index."),
        }
    }
}

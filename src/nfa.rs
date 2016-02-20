// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// This module implements the "NFA algorithm." That is, it guarantees linear
// time search of a regex on any text with memory use proportional to the size
// of the regex.
//
// It is equal in power to the backtracking engine in this crate, except the
// backtracking engine is typically faster on small regexes/texts at the
// expense of a bigger memory footprint.
//
// It can do more than the DFA can (specifically, record capture locations
// and execute word boundary assertions), but at a slower speed. Specifically,
// the NFA algorithm exectues a DFA implicitly by repeatedly expanding
// epsilon transitions. That is, the NFA engine can be in multiple states at
// once where as the DFA is only ever in one state at a time.
//
// Therefore, the NFA algorithm is generally treated as the fallback when the
// other matching engines either aren't feasible to run or are insufficient.

use std::mem;

use captures::{CaptureSlots, CaptureSlotsOwned, CaptureSlotsBorrowed};
use exec::Search;
use input::{Input, InputAt};
use inst::{InstPtr, InstSave};
use program::Program;
use sparse::SparseSet;

/// An NFA simulation matching engine.
#[derive(Debug)]
pub struct Nfa<'r, T> {
    /// The sequence of opcodes (among other things) that is actually executed.
    ///
    /// The program may be byte oriented or Unicode codepoint oriented.
    prog: &'r Program,
    /// An explicit stack used for following epsilon transitions. (This is
    /// borrowed from the cache.)
    stack: &'r mut Vec<FollowEpsilon>,
    /// Tracks matches seen.
    seen_matches: &'r mut SparseSet,
    /// The input text to search.
    text: T,
}

/// A cached allocation that can be reused on each execution.
#[derive(Debug)]
pub struct NfaCache {
    /// A pair of ordered sets for tracking NFA states.
    clist: Threads,
    nlist: Threads,
    /// An explicit stack used for following epsilon transitions.
    stack: Vec<FollowEpsilon>,
    /// A set used to track match status in each NFA state.
    ///
    /// This is only used when executing a regex set. Namely, once a match
    /// occurs for a particular regex in the set, we should not process any
    /// other NFA states for that regex on any particular byte.
    seen_matches: SparseSet,
}

/// An ordered set of NFA states and their captures.
#[derive(Debug)]
struct Threads {
    /// An ordered set of opcodes (each opcode is an NFA state).
    set: SparseSet,
    /// Captures for every NFA state.
    caps: Vec<Vec<Vec<Option<usize>>>>,
    /// The match slot of the most recently executed Save instruction for
    /// each thread.
    match_slots: Vec<Option<usize>>,
}

/// A representation of an explicit stack frame when following epsilon
/// transitions. This is used to avoid recursion.
#[derive(Debug)]
enum FollowEpsilon {
    /// Follow transitions at the given instruction pointer.
    IP(InstPtr),
    /// Restore the capture slot with the given position in the input.
    Capture {
        save: InstSave,
        old_match_slot: Option<usize>,
        old_capture_slot: Option<usize>,
    },
}

impl NfaCache {
    /// Create a new allocation used by the NFA machine to record execution
    /// and captures.
    pub fn new() -> Self {
        NfaCache {
            clist: Threads::new(),
            nlist: Threads::new(),
            stack: vec![],
            seen_matches: SparseSet::new(0),
        }
    }
}

impl<'r, T: Input> Nfa<'r, T> {
    /// Execute the NFA matching engine.
    ///
    /// If there's a match, `exec` returns `true` and populates the given
    /// captures accordingly.
    pub fn exec<'matches, C: CaptureSlots>(
        prog: &'r Program,
        text: T,
        start: usize,
        search: Search<'matches, C>,
    ) -> bool {
        let mut _cache = prog.cache_nfa();
        let mut cache = &mut **_cache;
        cache.clist.resize(prog);
        cache.nlist.resize(prog);
        if cache.seen_matches.capacity() != prog.insts.len() {
            cache.seen_matches = SparseSet::new(prog.insts.len());
        }
        let at = text.at(start);
        Nfa {
            prog: prog,
            stack: &mut cache.stack,
            seen_matches: &mut cache.seen_matches,
            text: text,
        }.exec_(&mut cache.clist, &mut cache.nlist, search, at)
    }

    fn exec_<'matches, C: CaptureSlots>(
        &mut self,
        mut clist: &mut Threads,
        mut nlist: &mut Threads,
        mut search: Search<'matches, C>,
        mut at: InputAt,
    ) -> bool {
        let mut matched = false;
        clist.set.clear();
        nlist.set.clear();
'LOOP:  loop {
            if clist.set.is_empty() {
                // Three ways to bail out when our current set of threads is
                // empty.
                //
                // 1. We have a match---so we're done exploring any possible
                //    alternatives.  Time to quit.
                //
                // 2. If the expression starts with a '^' we can terminate as
                //    soon as the last thread dies.
                if matched
                   || (!at.is_beginning() && self.prog.anchored_begin) {
                    break;
                }

                // 3. If there's a literal prefix for the program, try to
                //    jump ahead quickly. If it can't be found, then we can
                //    bail out early.
                if !self.prog.prefixes.is_empty() {
                    at = match self.text.prefix_at(&self.prog.prefixes, at) {
                        None => break,
                        Some(at) => at,
                    };
                }
            }

            // This simulates a preceding '.*?' for every regex by adding
            // a state starting at the current position in the input for the
            // beginning of the program only if we don't already have a match.
            if clist.set.is_empty()
                || (!self.prog.anchored_begin && !matched) {
                self.add(&mut clist, &mut search.captures, &mut None, 0, at)
            }
            // The previous call to "add" actually inspects the position just
            // before the current character. For stepping through the machine,
            // we can to look at the current character, so we advance the
            // input.
            let at_next = self.text.at(at.next_pos());
            self.seen_matches.clear();
            for i in 0..clist.set.len() {
                let ip = clist.set[i];
                if let Some(match_slot) = clist.match_slots[ip] {
                    if self.seen_matches.contains_ip(match_slot) {
                        continue;
                    }
                }
                let match_slot = clist.match_slots[ip];
                let m = self.step(
                    &mut search,
                    &mut nlist,
                    clist.caps(ip),
                    match_slot,
                    ip,
                    at,
                    at_next,
                );
                if m {
                    matched = true;
                    self.seen_matches.add(clist.match_slots[ip].unwrap());
                    if search.quit_after_first_match() {
                        // If we only care if a match occurs (not its
                        // position), then we can quit right now.
                        break 'LOOP;
                    }
                    // If we're running a regex set, then we need to traverse
                    // the rest of the states and fill in the captures for any
                    // proceding match states.
                    if search.matches.len() > 1 {
                        // self.set_matches(&mut search, clist, i);
                        // No breaking here. We must continue on to process
                        // all possible paths through the machine.
                    } else {
                        // We don't need to check the rest of the threads
                        // in this set because we've matched something
                        // ("leftmost-first"). However, we still need to check
                        // threads in the next set to support things like
                        // greedy matching.
                        break;
                    }
                }
            }
            if at.is_end() {
                break;
            }
            at = at_next;
            mem::swap(clist, nlist);
            nlist.set.clear();
        }
        matched
    }

    /// Step through the input, one token (byte or codepoint) at a time.
    ///
    /// nlist is the set of states that will be processed on the next token
    /// in the input.
    ///
    /// caps is the set of captures passed by the caller of the NFA. They are
    /// written to only when a match state is visited.
    ///
    /// thread_caps is the set of captures set for the current NFA state, ip.
    ///
    /// at and at_next are the current and next positions in the input. at or
    /// at_next may be EOF.
    fn step<'matches, C: CaptureSlots, D: CaptureSlots>(
        &mut self,
        search: &mut Search<'matches, D>,
        nlist: &mut Threads,
        thread_caps: C,
        mut match_slot: Option<usize>,
        ip: usize,
        at: InputAt,
        at_next: InputAt,
    ) -> bool {
        use inst::Inst::*;
        match self.prog.insts[ip] {
            Match(match_slot) => {
                search.captures.copy_from_match(&thread_caps, match_slot);
                search.matches[match_slot] = true;
                true
            }
            Char(ref inst) => {
                if inst.c == at.char() {
                    self.add(nlist, thread_caps, &mut match_slot, inst.goto, at_next);
                }
                false
            }
            Ranges(ref inst) => {
                if inst.matches(at.char()) {
                    self.add(nlist, thread_caps, &mut match_slot, inst.goto, at_next);
                }
                false
            }
            Bytes(ref inst) => {
                if let Some(b) = at.byte() {
                    if inst.matches(b) {
                        self.add(nlist, thread_caps, &mut match_slot, inst.goto, at_next);
                    }
                }
                false
            }
            EmptyLook(_) | Save(_) | Split(_) => false,
        }
    }

    /// Follows epsilon transitions and adds them for processing to nlist,
    /// starting at and including ip.
    ///
    /// N.B. The inline(always) appears to increase throughput by about
    /// 20% on micro-benchmarks.
    #[inline(always)]
    fn add<C: CaptureSlots>(
        &mut self,
        nlist: &mut Threads,
        mut thread_caps: C,
        mut match_slot: &mut Option<usize>,
        ip: usize,
        at: InputAt,
    ) {
        self.stack.push(FollowEpsilon::IP(ip));
        while let Some(frame) = self.stack.pop() {
            match frame {
                FollowEpsilon::IP(ip) => {
                    self.add_step(nlist, &mut thread_caps, match_slot, ip, at);
                }
                FollowEpsilon::Capture {
                    save,
                    old_match_slot,
                    old_capture_slot,
                } => {
                    *match_slot = old_match_slot;
                    thread_caps.set_capture(
                        save.match_slot, save.capture_slot, old_capture_slot);
                }
            }
        }
    }

    /// A helper function for add that avoids excessive pushing to the stack.
    fn add_step<C: CaptureSlots>(
        &mut self,
        nlist: &mut Threads,
        mut thread_caps: C,
        mut match_slot: &mut Option<usize>,
        mut ip: usize,
        at: InputAt,
    ) {
        // Instead of pushing and popping to the stack, we mutate ip as we
        // traverse the set of states. We only push to the stack when we
        // absolutely need recursion (restoring captures or following a
        // branch).
        use inst::Inst::*;
        loop {
            // Don't visit states we've already added.
            if nlist.set.contains_ip(ip) {
                return;
            }
            nlist.set.add(ip);
            match self.prog.insts[ip] {
                EmptyLook(ref inst) => {
                    let prev = self.text.previous_char(at);
                    let next = self.text.next_char(at);
                    if inst.matches(prev, next) {
                        ip = inst.goto;
                    }
                }
                Save(ref inst) => {
                    let cap = thread_caps.capture(
                        inst.match_slot, inst.capture_slot);
                    if let Some(capture_slot) = cap {
                        self.stack.push(FollowEpsilon::Capture {
                            save: *inst,
                            old_match_slot: *match_slot,
                            old_capture_slot: capture_slot,
                        });
                        thread_caps.set_capture(
                            inst.match_slot,
                            inst.capture_slot,
                            Some(at.pos()));
                    }
                    *match_slot = Some(inst.match_slot);
                    ip = inst.goto;
                }
                Split(ref inst) => {
                    self.stack.push(FollowEpsilon::IP(inst.goto2));
                    ip = inst.goto1;
                }
                Match(_) | Char(_) | Ranges(_) | Bytes(_) => {
                    nlist.caps(ip).copy_from(&thread_caps);
                    nlist.match_slots[ip] = *match_slot;
                    return;
                }
            }
        }
    }

    /// Copy match captures to the rest of the match instructions in clist.
    ///
    /// The first match instruction should be indexed by thread_last_match.
    fn set_matches<'matches, C: CaptureSlots>(
        &self,
        search: &mut Search<'matches, C>,
        clist: &mut Threads,
        thread_last_match: usize,
    ) {
        use inst::Inst::*;
        for i in thread_last_match+1..clist.set.len() {
            let ip = clist.set[i];
            if let Match(match_slot) = self.prog.insts[ip] {
                search.captures.copy_from_match(&clist.caps(ip), match_slot);
                search.matches[match_slot] = true;
            }
        }
    }
}

impl Threads {
    fn new() -> Self {
        Threads {
            set: SparseSet::new(0),
            caps: vec![],
            match_slots: vec![],
        }
    }

    // fn resize(&mut self, num_insts: usize, ncaps: usize) {
    fn resize(&mut self, prog: &Program) {
        if prog.insts.len() == self.set.capacity() {
            return;
        }
        self.set = SparseSet::new(prog.insts.len());
        self.caps = vec![prog.alloc_captures(); prog.insts.len()];
        self.match_slots = vec![None; prog.insts.len()];
    }

    fn caps(&mut self, pc: usize) -> &mut [Vec<Option<usize>>] {
        &mut *self.caps[pc]
    }
}

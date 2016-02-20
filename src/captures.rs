use std::cmp;
use std::fmt;

pub type CaptureSlot = Option<usize>;

pub trait CaptureSlots: fmt::Debug {
    fn num_matches(&self) -> usize;
    fn capture(&self, ms: usize, cs: usize) -> Option<CaptureSlot>;
    fn set_capture(&mut self, ms: usize, cs: usize, slot: CaptureSlot);
    fn captures(&self, ms: usize) -> &[CaptureSlot];
    fn captures_mut(&mut self, ms: usize) -> &mut [CaptureSlot];

    fn copy_from<T: CaptureSlots>(&mut self, other: &T) {
        for ms in 0..cmp::min(self.num_matches(), other.num_matches()) {
            self.copy_from_match(other, ms)
        }
    }

    fn copy_from_match<T: CaptureSlots>(&mut self, other: &T, ms: usize) {
        let (dst, src) = (self.captures_mut(ms), other.captures(ms));
        for (slot, val) in dst.iter_mut().zip(src.iter()) {
            *slot = *val;
        }
    }
}

pub type CaptureSlotsOwned = Vec<Vec<CaptureSlot>>;
pub type CaptureSlotsBorrowed<'a> = &'a mut [&'a mut [CaptureSlot]];

impl CaptureSlots for CaptureSlotsOwned {
    fn num_matches(&self) -> usize {
        self.len()
    }

    fn capture(&self, ms: usize, cs: usize) -> Option<CaptureSlot> {
        self.get(ms).and_then(|slots| slots.get(cs)).map(|slot| *slot)
    }

    fn set_capture(&mut self, ms: usize, cs: usize, slot: CaptureSlot) {
        self[ms][cs] = slot;
    }

    fn captures(&self, ms: usize) -> &[CaptureSlot] {
        &self[ms]
    }

    fn captures_mut(&mut self, ms: usize) -> &mut [CaptureSlot] {
        &mut self[ms]
    }
}

impl<'a> CaptureSlots for [&'a mut [CaptureSlot]] {
    fn num_matches(&self) -> usize {
        self.len()
    }

    fn capture(&self, ms: usize, cs: usize) -> Option<CaptureSlot> {
        self.get(ms).and_then(|slots| slots.get(cs)).map(|slot| *slot)
    }

    fn set_capture(&mut self, ms: usize, cs: usize, slot: CaptureSlot) {
        self[ms][cs] = slot;
    }

    fn captures(&self, ms: usize) -> &[CaptureSlot] {
        &self[ms]
    }

    fn captures_mut(&mut self, ms: usize) -> &mut [CaptureSlot] {
        &mut self[ms]
    }
}

impl<'a> CaptureSlots for &'a mut [&'a mut [CaptureSlot]] {
    fn num_matches(&self) -> usize {
        self.len()
    }

    fn capture(&self, ms: usize, cs: usize) -> Option<CaptureSlot> {
        self.get(ms).and_then(|slots| slots.get(cs)).map(|slot| *slot)
    }

    fn set_capture(&mut self, ms: usize, cs: usize, slot: CaptureSlot) {
        self[ms][cs] = slot;
    }

    fn captures(&self, ms: usize) -> &[CaptureSlot] {
        &self[ms]
    }

    fn captures_mut(&mut self, ms: usize) -> &mut [CaptureSlot] {
        &mut self[ms]
    }
}

impl<'a> CaptureSlots for &'a mut [Vec<CaptureSlot>] {
    fn num_matches(&self) -> usize {
        self.len()
    }

    fn capture(&self, ms: usize, cs: usize) -> Option<CaptureSlot> {
        self.get(ms).and_then(|slots| slots.get(cs)).map(|slot| *slot)
    }

    fn set_capture(&mut self, ms: usize, cs: usize, slot: CaptureSlot) {
        self[ms][cs] = slot;
    }

    fn captures(&self, ms: usize) -> &[CaptureSlot] {
        &self[ms]
    }

    fn captures_mut(&mut self, ms: usize) -> &mut [CaptureSlot] {
        &mut self[ms]
    }
}

impl<'a, C: CaptureSlots> CaptureSlots for &'a mut C {
    fn num_matches(&self) -> usize {
        (**self).num_matches()
    }

    fn capture(&self, ms: usize, cs: usize) -> Option<CaptureSlot> {
        (**self).capture(ms, cs)
    }

    fn set_capture(&mut self, ms: usize, cs: usize, slot: CaptureSlot) {
        (**self).set_capture(ms, cs, slot);
    }

    fn captures(&self, ms: usize) -> &[CaptureSlot] {
        (**self).captures(ms)
    }

    fn captures_mut(&mut self, ms: usize) -> &mut [CaptureSlot] {
        (**self).captures_mut(ms)
    }
}

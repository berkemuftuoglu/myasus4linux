//! Debounced optimistic commit for slider-driven controls.
//!
//! A slider fires a value per drag step; we want one privileged write per
//! *settle*, shown optimistically meanwhile, rolled back if the write fails, and
//! not stomped by the background poll while a write is in flight. That little
//! state machine was copy-pasted across the charge and screen sliders (and had
//! already drifted -- the screen copy lost its poll guard). It lives here once,
//! GTK-free and unit-tested; the page owns only the glib timeout + the write.

pub struct DebouncedCommit<T> {
    value: T,     // displayed / desired
    committed: T, // last value confirmed written
    seq: u32,     // generation, to drop superseded commits
    pending: bool,
}

impl<T: Copy + PartialEq> DebouncedCommit<T> {
    pub fn new(initial: T) -> Self {
        Self {
            value: initial,
            committed: initial,
            seq: 0,
            pending: false,
        }
    }

    pub fn value(&self) -> T {
        self.value
    }

    /// The slider moved. Update the displayed value and, if it actually changed,
    /// return the generation token to schedule a commit for.
    pub fn slide(&mut self, v: T) -> Option<u32> {
        if v == self.value {
            return None;
        }
        self.value = v;
        self.pending = true;
        self.seq = self.seq.wrapping_add(1);
        Some(self.seq)
    }

    /// A scheduled commit fired. Returns the value to write if this generation is
    /// still current and differs from what's committed; otherwise it's superseded
    /// or a no-op and nothing should be written.
    pub fn commit(&mut self, seq: u32) -> Option<T> {
        if seq != self.seq {
            return None; // a newer move superseded this one
        }
        if self.value == self.committed {
            self.pending = false;
            return None;
        }
        Some(self.value)
    }

    /// The write finished. On success the displayed value becomes the committed
    /// one; on failure the displayed value rolls back to the last good value.
    pub fn written(&mut self, ok: bool) {
        self.pending = false;
        if ok {
            self.committed = self.value;
        } else {
            self.value = self.committed;
        }
    }

    /// Adopt a value read by the background poll, unless a write is in flight
    /// (which would otherwise stomp the optimistic value mid-drag).
    pub fn poll(&mut self, v: T) {
        if !self.pending {
            self.value = v;
            self.committed = v;
        }
    }
}

/// Optimistic discrete choice (the profile toggles): show the picked value at
/// once, roll back if the write fails, and don't let the poll stomp it while a
/// write is in flight. The same state machine `Overview` and `FanPage` carried.
pub struct OptimisticChoice<T> {
    current: T,
    pending_prev: Option<T>,
}

impl<T: Copy + PartialEq> OptimisticChoice<T> {
    pub fn new(initial: T) -> Self {
        Self {
            current: initial,
            pending_prev: None,
        }
    }

    pub fn current(&self) -> T {
        self.current
    }

    /// User picked `p`. Returns `Some(p)` to write if it actually changed,
    /// applying it optimistically and marking a write pending.
    pub fn pick(&mut self, p: T) -> Option<T> {
        if p == self.current {
            return None;
        }
        self.pending_prev = Some(self.current);
        self.current = p;
        Some(p)
    }

    /// The write finished; roll back to the previous value on failure.
    pub fn written(&mut self, ok: bool) {
        if let Some(prev) = self.pending_prev.take() {
            if !ok {
                self.current = prev;
            }
        }
    }

    /// Adopt a polled value unless our own write is in flight.
    pub fn poll(&mut self, p: T) {
        if self.pending_prev.is_none() {
            self.current = p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_optimistic_pick_and_rollback() {
        let mut c = OptimisticChoice::new(0u8);
        assert_eq!(c.pick(0), None); // unchanged
        assert_eq!(c.pick(1), Some(1)); // optimistic
        assert_eq!(c.current(), 1);
        c.poll(2); // stale poll mid-write -> ignored
        assert_eq!(c.current(), 1);
        c.written(false); // write failed -> roll back
        assert_eq!(c.current(), 0);
        c.poll(2); // now allowed
        assert_eq!(c.current(), 2);
    }

    #[test]
    fn supersede_drops_the_older_commit() {
        let mut c = DebouncedCommit::new(60u8);
        let s1 = c.slide(70).unwrap();
        let s2 = c.slide(80).unwrap();
        assert_ne!(s1, s2);
        assert_eq!(c.commit(s1), None); // superseded
        assert_eq!(c.commit(s2), Some(80));
    }

    #[test]
    fn commit_is_noop_when_unchanged() {
        let mut c = DebouncedCommit::new(60u8);
        let s = c.slide(80).unwrap();
        c.slide(60); // dragged back to the committed value
        assert_eq!(c.commit(s), None);
    }

    #[test]
    fn failed_write_rolls_back_displayed_value() {
        let mut c = DebouncedCommit::new(60u8);
        let s = c.slide(80).unwrap();
        assert_eq!(c.commit(s), Some(80));
        c.written(false);
        assert_eq!(c.value(), 60); // rolled back
    }

    #[test]
    fn poll_is_ignored_while_pending() {
        let mut c = DebouncedCommit::new(60u8);
        c.slide(80);
        c.poll(55); // a stale background read mid-drag
        assert_eq!(c.value(), 80);
        c.written(true);
        c.poll(55); // now allowed
        assert_eq!(c.value(), 55);
    }
}

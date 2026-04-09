use std::cmp::Ordering;
use std::collections::HashMap;
use std::task::Waker;
use std::time::Instant;

use crate::rt::time::{RawHandle, TimerEntry, TimerHandle};

/// Indexed priority queue for timer management.
///
/// Maintains the property where the timer with the earliest deadline is at the
/// root, and each parent timer's deadline is earlier than or equal to that of
/// its children.
#[derive(Debug, Clone)]
pub struct TimerHeap {
    // Mapping from a `RawHandle` to its position in `buf`.
    handles: HashMap<RawHandle, usize>,
    // Contiguous buffer used for better cache-locality and index-based access.
    buf: Vec<TimerEntry>,
}

/// Iterator that yields mutable references over the entries of a `TimerHeap` in
/// heap-order, restoring its prior state on `Drop`.
#[derive(Debug)]
pub struct HeapIter<'a> {
    heap: &'a mut TimerHeap,
    curr: usize,
}

impl HeapIter<'_> {
    #[must_use]
    pub fn next_entry(&mut self) -> Option<&mut TimerEntry> {
        if self.curr >= self.heap.len() {
            return None;
        }

        self.curr += 1;
        let tail = self.heap.len() - self.curr;

        self.heap.buf.swap(0, tail);
        self.heap.sift_down(0, tail);

        Some(&mut self.heap.buf[tail])
    }
}

impl Drop for HeapIter<'_> {
    fn drop(&mut self) {
        if !self.heap.is_empty() {
            let n = self.heap.len();
            let tail = self.heap.len() - self.curr;

            // If few elements were iterated, it’s cheaper to `sift_up` each
            // one individually. O(k log n) where `k` is the number of entries
            // iterated over.
            if tail * (usize::ilog2(n) as usize) < n {
                for i in tail..n {
                    self.heap.sift_up(0, i);
                }
            } else {
                self.heap.rebuild();
            }
        }
    }
}

/// Encodes the index of the item to "sift" from and in which direction. The
/// upper 63 bits store the index, and the lowest bit indicates the direction:
/// `1` for upward (sift-up), `0` for downward (sift-down).
#[repr(transparent)]
struct SiftInfo(u64);

impl SiftInfo {
    #[inline]
    const fn new(pos: usize, should_sift_up: bool) -> Self {
        // Rust collections limits allocations to [`isize::MAX`], which fits
        // within the lower 63 bits of a `u64`, meaning all `pos` values can be
        // correctly encoded.
        let packed = (pos as u64) | ((should_sift_up as u64) << 63);

        SiftInfo(packed)
    }

    #[inline]
    const fn pos(&self) -> usize {
        (self.0 & !(1 << 63)) as usize
    }

    #[inline]
    const fn sift_up(&self) -> bool {
        ((self.0 >> 63) & 0x1) != 0
    }
}

// <https://doc.rust-lang.org/src/alloc/collections/binary_heap/mod.rs.html#484>
struct HeapifyGuard<'a> {
    heap: &'a mut TimerHeap,
    sift_info: SiftInfo,
}

impl Drop for HeapifyGuard<'_> {
    fn drop(&mut self) {
        let pos = self.sift_info.pos();

        if self.sift_info.sift_up() {
            debug_assert!(
                pos < self.heap.len(),
                "invalid range `0..={pos}` when sifting up, heap's range is `0..{}`",
                self.heap.len()
            );

            self.heap.sift_up(0, pos);
        } else {
            debug_assert!(
                pos <= self.heap.len(),
                "invalid range `0..{pos}` when sifting down, heap's range is `0..{}`",
                self.heap.len()
            );

            self.heap.sift_down(0, pos);
        }
    }
}

impl TimerHeap {
    #[must_use]
    pub fn new() -> Self {
        TimerHeap {
            handles: HashMap::default(),
            buf: Vec::default(),
        }
    }

    pub fn push(&mut self, deadline: Instant, waker: Waker) -> TimerHandle {
        let guard = HeapifyGuard {
            // Item to sift up will be at this index after `push`.
            sift_info: SiftInfo::new(self.len(), true),
            heap: self,
        };

        let handle = TimerHandle::new();

        // Appending maintains the invariant of a complete binary tree: every
        // level, except possibly the last, is fully filled.
        guard
            .heap
            .buf
            .push(TimerEntry::new(deadline, waker, handle.raw()));

        handle

        // `guard` rebuilds the heap on `Drop`...
    }

    #[allow(unused)]
    pub fn pop(&mut self) -> Option<TimerEntry> {
        if self.is_empty() {
            None
        } else {
            let guard = HeapifyGuard {
                // Where `sift_down` should stop after `swap_remove`.
                sift_info: SiftInfo::new(self.len() - 1, false),
                heap: self,
            };

            let timer = guard.heap.buf.swap_remove(0);

            guard.heap.handles.remove(&timer.raw_handle);

            Some(timer)

            // `guard` rebuilds the heap on `Drop`...
        }
    }

    #[allow(unused)]
    pub fn update_priority(&mut self, handle: &TimerHandle, deadline: Instant) -> bool {
        if let Some(&idx) = self.handles.get(&handle.raw()) {
            let timer = &mut self.buf[idx];

            return match timer.deadline.cmp(&deadline) {
                Ordering::Less => {
                    timer.deadline = deadline;
                    self.sift_down(idx, self.len());
                    true
                }
                Ordering::Equal => false,
                Ordering::Greater => {
                    timer.deadline = deadline;
                    self.sift_up(0, idx);
                    true
                }
            };
        }

        false
    }

    /// # Panics
    ///
    /// Panics if `idx >= len`.
    pub fn update_priority_with_idx(&mut self, idx: usize, deadline: Instant) -> bool {
        let timer = &mut self.buf[idx];

        match timer.deadline.cmp(&deadline) {
            Ordering::Less => {
                timer.deadline = deadline;
                self.sift_down(idx, self.len());
                true
            }
            Ordering::Equal => false,
            Ordering::Greater => {
                timer.deadline = deadline;
                self.sift_up(0, idx);
                true
            }
        }
    }

    pub fn remove(&mut self, handle: &TimerHandle) -> Option<TimerEntry> {
        if let Some(idx) = self.handles.remove(&handle.raw()) {
            let timer = self.buf.swap_remove(idx);

            if !self.is_empty() {
                self.sift_down(idx, self.len());
            }

            return Some(timer);
        }

        None
    }

    pub const fn heap_iter(&mut self) -> HeapIter<'_> {
        HeapIter {
            heap: self,
            curr: 0,
        }
    }

    pub fn get_mut(&mut self, handle: &TimerHandle) -> Option<(&mut TimerEntry, usize)> {
        if let Some(&idx) = self.handles.get(&handle.raw()) {
            return Some((&mut self.buf[idx], idx));
        }

        None
    }

    #[allow(unused)]
    pub fn peek(&self) -> Option<&TimerEntry> {
        self.buf.first()
    }

    pub const fn len(&self) -> usize {
        self.buf.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Restores the min-heap invariant for the entire heap, fixing any
    /// violations.
    fn rebuild(&mut self) {
        let mut pos = self.len() / 2;

        while pos > 0 {
            pos -= 1;

            self.sift_down(pos, self.len());
        }
    }

    /// Restores the min-heap invariant by fixing any violations caused after
    /// an insertion, returning the new position of the item.
    ///
    /// `start` specifies the upper bound (inclusive) for where sifting should
    /// stop. `pos` is the index of the item being moved.
    ///
    /// # Panics
    ///
    /// Panics if `start..=pos` doesn't lie entirely within the bounds of the
    /// heap.
    fn sift_up(&mut self, start: usize, mut pos: usize) -> usize {
        // For an element at index `i`:
        //
        // - Parent: (i - 1) / 2
        while pos > start {
            let parent = (pos - 1) / 2;

            if self.buf[pos] >= self.buf[parent] {
                break;
            }

            // Swap item at `pos` with its parent.
            self.buf.swap(pos, parent);

            self.handles.insert(self.buf[parent].raw_handle, parent);

            pos = parent;
        }

        self.handles.insert(self.buf[pos].raw_handle, pos);

        pos
    }

    /// Restores the min-heap invariant by fixing any violations caused after
    /// a removal, returning the new position of the item.
    ///
    /// `pos` is the index of the item that is being moved. `end` specifies the
    /// upper bound (exclusive) for where the sifting should stop.     
    ///
    /// # Panics
    ///
    /// Panics if `pos..end` doesn't lie entirely within the bounds of the heap.
    fn sift_down(&mut self, mut pos: usize, end: usize) -> usize {
        // For an element at index `i`:
        //
        // - Left child:  2i + 1
        // - Right child: 2i + 2
        loop {
            let left = 2 * pos + 1;
            let right = 2 * pos + 2;

            // Comparison must start with the left child.
            if left >= end {
                break;
            }

            let mut min = if self.buf[pos] >= self.buf[left] {
                left
            } else {
                pos
            };

            // Check if the right child exists before comparing.
            if right < end && self.buf[min] >= self.buf[right] {
                min = right;
            }

            // Check if a "smaller" child was encountered.
            if min == pos {
                // Can no longer sift down.
                break;
            } else {
                self.buf.swap(min, pos);

                self.handles.insert(self.buf[min].raw_handle, min);

                pos = min;
            }
        }

        if pos < self.len() {
            self.handles.insert(self.buf[pos].raw_handle, pos);
        }

        pos
    }
}

impl Default for TimerHeap {
    fn default() -> Self {
        TimerHeap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::task::Waker;
    use std::time::Duration;

    // NOTE: Tests are wrapped in `rt!` macro since `TimerHandle::Drop` relies
    // on a runtime context to cancel associated timers. We create a separate
    // `TimerHeap` instance for testing the heap logic, but the Drop impl still
    // attempts to cancel on the runtime's `TimerHeap`, effectively a noop.

    #[test]
    fn test_new() {
        let heap = TimerHeap::new();

        assert_eq!(heap.len(), 0);
        assert!(heap.is_empty());
        assert_eq!(heap.peek(), None);
    }

    #[test]
    fn test_push_one() {
        rt! {
            let mut heap = TimerHeap::new();

            let deadline1 = Instant::now() + Duration::from_secs(1);
            let _h1 = heap.push(deadline1, Waker::noop().clone());

            assert_eq!(heap.len(), 1);
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadline1);
        }
    }

    #[test]
    fn test_pop_one() {
        rt! {
            let mut heap = TimerHeap::new();

            let deadline1 = Instant::now() + Duration::from_secs(1);
            let _h1 = heap.push(deadline1, Waker::noop().clone());

            assert_eq!(heap.len(), 1);
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadline1);

            let popped = heap.pop();

            assert_eq!(heap.len(), 0);
            assert!(heap.is_empty());
            assert_eq!(popped.expect("heap should be non-empty").deadline, deadline1);
            assert_eq!(heap.peek(), None);
        }
    }

    #[test]
    fn test_push_many() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let deadlines = [
                now + Duration::from_secs(5),
                now + Duration::from_secs(7),
                now + Duration::from_secs(2),
                now + Duration::from_secs(4),
                now + Duration::from_secs(1),
            ];

            let _handles: Vec<_> = deadlines
                .iter()
                .map(|d| heap.push(*d, Waker::noop().clone()))
                .collect();

            assert_eq!(heap.len(), deadlines.len());
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, *deadlines.last().expect("heap should be non-empty"));
        }
    }

    #[test]
    fn test_pop_many() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let mut deadlines = [
                now + Duration::from_secs(5),
                now + Duration::from_secs(7),
                now + Duration::from_secs(2),
                now + Duration::from_secs(4),
                now + Duration::from_secs(1),
            ];

            let _handles: Vec<_> = deadlines
                .iter()
                .map(|d| heap.push(*d, Waker::noop().clone()))
                .collect();

            assert_eq!(heap.len(), deadlines.len());
            assert!(!heap.is_empty());

            deadlines.sort();

            for deadline in deadlines {
                assert_eq!(heap.pop().expect("heap should be non-empty").deadline, deadline);
            }

            assert_eq!(heap.len(), 0);
            assert!(heap.is_empty());
            assert_eq!(heap.pop(), None);
        }
    }

    #[test]
    fn test_push_duplicate() {
        rt! {
            let mut heap = TimerHeap::new();
            let deadline = Instant::now() + Duration::from_secs(5);

            let h1 = heap.push(deadline, Waker::noop().clone());
            let h2 = heap.push(deadline, Waker::noop().clone());

            assert_eq!(heap.len(), 2);
            assert!(!heap.is_empty());
            assert_ne!(h1, h2);

            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadline);

            let entry1 = heap.pop().expect("heap should be non-empty");
            assert_eq!(entry1.deadline, deadline);

            let entry2 = heap.pop().expect("heap should be non-empty");
            assert_eq!(entry2.deadline, deadline);

            assert_eq!(heap.len(), 0);
            assert!(heap.is_empty());
            assert_eq!(heap.peek(), None);
        }
    }

    #[test]
    fn test_update_earlier() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let deadlines = [
                now + Duration::from_secs(10),
                now + Duration::from_secs(5),
            ];

            let h1 = heap.push(deadlines[0], Waker::noop().clone());
            let _h2 = heap.push(deadlines[1], Waker::noop().clone());

            assert_eq!(heap.len(), deadlines.len());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[1]);

            let new_deadline = Instant::now() + Duration::from_secs(2);
            assert!(heap.update_priority(&h1, new_deadline));

            assert_eq!(heap.len(), deadlines.len());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, new_deadline);
        }
    }

    #[test]
    fn test_update_later() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let deadlines = [
                now + Duration::from_secs(5),
                now + Duration::from_secs(7),
            ];

            let h1 = heap.push(deadlines[0], Waker::noop().clone());
            let _h2 = heap.push(deadlines[1], Waker::noop().clone());

            assert_eq!(heap.len(), deadlines.len());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[0]);

            let new_deadline = Instant::now() + Duration::from_secs(10);
            assert!(heap.update_priority(&h1, new_deadline));

            assert_eq!(heap.len(), deadlines.len());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[1]);
        }
    }

    #[test]
    fn test_remove_all() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let deadlines = [
                now + Duration::from_secs(5),
                now + Duration::from_secs(7),
                now + Duration::from_secs(10),
            ];

            let h1 = heap.push(deadlines[0], Waker::noop().clone());
            let h2 = heap.push(deadlines[1], Waker::noop().clone());
            let h3 = heap.push(deadlines[2], Waker::noop().clone());

            assert_eq!(heap.len(), deadlines.len());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[0]);

            assert!(heap.remove(&h2).is_some());
            assert_eq!(heap.len(), 2);
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[0]);

            assert!(heap.remove(&h1).is_some());
            assert_eq!(heap.len(), 1);
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[2]);

            assert!(heap.remove(&h3).is_some());
            assert_eq!(heap.len(), 0);
            assert!(heap.is_empty());
            assert_eq!(heap.peek(), None);
        }
    }

    #[test]
    fn test_remove_invalid() {
        rt! {
            let mut heap = TimerHeap::new();
            let h1 = heap.push(Instant::now(), Waker::noop().clone());

            assert!(heap.remove(&h1).is_some());
            assert!(heap.remove(&h1).is_none());
        }
    }

    #[test]
    fn test_remove_middle() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let deadlines = [
                now + Duration::from_secs(1),
                now + Duration::from_secs(5),
                now + Duration::from_secs(10),
            ];

            let handles: Vec<_> = deadlines
                .iter()
                .map(|d| heap.push(*d, Waker::noop().clone()))
                .collect();

            assert_eq!(heap.len(), 3);
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[0]);

            assert!(heap.remove(&handles[1]).is_some());
            assert_eq!(heap.len(), 2);

            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, deadlines[0]);
            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, deadlines[2]);
            assert!(heap.pop().is_none());

            for handle in &handles {
                assert!(heap.remove(handle).is_none());
            }
        }
    }

    #[test]
    fn test_remove_root_then_pop() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let h1 = heap.push(now + Duration::from_secs(1), Waker::noop().clone());
            let _h2 = heap.push(now + Duration::from_secs(2), Waker::noop().clone());
            let _h3 = heap.push(now + Duration::from_secs(3), Waker::noop().clone());

            assert!(heap.remove(&h1).is_some());
            assert_eq!(heap.len(), 2);

            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, now + Duration::from_secs(2));
            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, now + Duration::from_secs(3));

            assert_eq!(heap.len(), 0);
            assert!(heap.is_empty());
            assert_eq!(heap.peek(), None);
        }
    }

    #[test]
    fn test_remove_last() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let _h1 = heap.push(now + Duration::from_secs(1), Waker::noop().clone());
            let _h2 = heap.push(now + Duration::from_secs(2), Waker::noop().clone());
            let h3 = heap.push(now + Duration::from_secs(3), Waker::noop().clone());

            assert!(heap.remove(&h3).is_some());
            assert_eq!(heap.len(), 2);

            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, now + Duration::from_secs(1));
            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, now + Duration::from_secs(2));

            assert_eq!(heap.len(), 0);
            assert!(heap.is_empty());
            assert_eq!(heap.peek(), None);
        }
    }

    #[test]
    fn test_pop_expiration_order() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let deadlines = [
                now + Duration::from_secs(5),
                now + Duration::from_secs(1),
                now + Duration::from_secs(3),
                now + Duration::from_secs(2),
            ];

            for deadline in deadlines {
                heap.push(deadline, Waker::noop().clone().clone());
            }

            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, deadlines[1]);
            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, deadlines[3]);
            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, deadlines[2]);
            assert_eq!(heap.pop().expect("heap should be non-empty").deadline, deadlines[0]);

            assert_eq!(heap.len(), 0);
            assert!(heap.is_empty());
            assert_eq!(heap.peek(), None);
        }
    }

    #[test]
    fn test_heap_iter_basic() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let deadlines = [
                now + Duration::from_secs(1),
                now + Duration::from_secs(3),
                now + Duration::from_secs(2),
            ];

            for deadline in deadlines {
                heap.push(deadline, Waker::noop().clone());
            }

            let mut iter = heap.heap_iter();

            assert_eq!(iter.next_entry().expect("heap should be non-empty").deadline, deadlines[0]);
            assert_eq!(iter.next_entry().expect("heap should be non-empty").deadline, deadlines[2]);
            assert_eq!(iter.next_entry().expect("heap should be non-empty").deadline, deadlines[1]);
            assert!(iter.next_entry().is_none());

            drop(iter);

            assert_eq!(heap.len(), 3);
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[0]);
        }
    }

    #[test]
    fn test_heap_iter_partial_drop() {
        rt! {
            let mut heap = TimerHeap::new();
            let deadline1 = Instant::now() + Duration::from_secs(5);
            let deadline2 = Instant::now() + Duration::from_secs(10);

            heap.push(deadline1, Waker::noop().clone());
            heap.push(deadline2, Waker::noop().clone());

            assert_eq!(heap.len(), 2);
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadline1);

            let mut heap_clone = heap.clone();

            {
                let mut iter = heap.heap_iter();
                assert_eq!(iter.next_entry().expect("heap should be non-empty").deadline, deadline1);
            }

            assert_eq!(heap.len(), 2);
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadline1);

            for _ in 0..heap.len() {
                assert_eq!(heap.pop(), heap_clone.pop());
            }
        }
    }

    #[test]
    fn test_heap_iter_full_drop() {
        rt! {
            let mut heap = TimerHeap::new();
            let now = Instant::now();

            let deadlines = [
                now + Duration::from_secs(1),
                now + Duration::from_secs(3),
                now + Duration::from_secs(2),
            ];

            for deadline in deadlines {
                heap.push(deadline, Waker::noop().clone());
            }

            assert_eq!(heap.len(), 3);
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[0]);

            let mut heap_clone = heap.clone();

            {
                let mut iter = heap.heap_iter();
                while iter.next_entry().is_some() {}
            }

            assert_eq!(heap.len(), 3);
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().expect("heap should be non-empty").deadline, deadlines[0]);

            for _ in 0..heap.len() {
                assert_eq!(heap.pop(), heap_clone.pop());
            }
        }
    }

    #[test]
    fn test_heap_iter_empty() {
        rt! {
            let mut heap = TimerHeap::new();
            let mut iter = heap.heap_iter();
            assert!(iter.next_entry().is_none());
            drop(iter);
            assert_eq!(heap.len(), 0);
        }
    }
}

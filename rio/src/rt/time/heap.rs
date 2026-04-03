use std::task::Waker;
use std::time::Instant;
use std::{cmp::Ordering, collections::HashMap};

use crate::rt::time::{RawHandle, TimerEntry, TimerHandle};

/// Indexed priority queue for timer management.
///
/// Maintains the property where the timer with the earliest deadline is at the
/// root, and each parent timer's deadline is earlier than or equal to that of
/// its children.
#[derive(Debug)]
pub struct TimerHeap {
    // Mapping from a `RawHandle` to its position in `buf`.
    handles: HashMap<RawHandle, usize>,
    // Contiguous buffer used for better cache-locality and index-based access.
    buf: Vec<TimerEntry>,
}

/// Allows for yielding references to the entries of a `TimerHeap` in heap-order
/// and restores the heap on `Drop`.
#[derive(Debug)]
pub struct HeapIter<'a> {
    heap: &'a mut TimerHeap,
    curr: usize,
}

impl HeapIter<'_> {
    pub fn next(&self) -> Option<&TimerEntry> {
        if self.is_exhausted() {
            return None;
        }

        self.heap.peek()
    }

    pub fn set_next(&mut self) {
        self.curr += 1;

        let end = self.heap.len() - self.curr;

        self.heap.buf.swap(0, end);
        self.heap.sift_down(0, end);
    }

    const fn is_exhausted(&self) -> bool {
        self.curr >= self.heap.len()
    }
}

impl Drop for HeapIter<'_> {
    fn drop(&mut self) {
        self.heap.rebuild();
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
        // NOTE: Rust collections limits allocations to [`isize::MAX`], which
        // fits within the lower 63 bits of a `u64`, meaning all `pos` values
        // can be correctly encoded.
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
        // Item to sift up will be at this index after `push`.
        let pos = self.len();
        let guard = HeapifyGuard {
            heap: self,
            sift_info: SiftInfo::new(pos, true),
        };

        let handle = TimerHandle::new();
        let raw_handle = handle.raw();

        // Appending maintains the invariant of a complete binary tree: every
        // level, except possibly the last, is fully filled.
        guard.heap.buf.push(TimerEntry {
            deadline,
            waker,
            raw_handle,
        });

        guard.heap.handles.insert(raw_handle, pos);

        handle

        // `guard` rebuilds the heap on `Drop`...
    }

    #[allow(unused)]
    pub fn pop(&mut self) -> Option<TimerEntry> {
        if self.is_empty() {
            None
        } else {
            // Length of the heap after `swap_remove`.
            let end = self.len() - 1;
            let guard = HeapifyGuard {
                sift_info: SiftInfo::new(end, false),
                heap: self,
            };

            // NOTE: O(1) time, instead of `remove(0)` which is O(n).
            let timer = guard.heap.buf.swap_remove(0);

            guard.heap.handles.remove(&timer.raw_handle);

            Some(timer)

            // `guard` rebuilds the heap on `Drop`...
        }
    }

    pub fn update_priority(&mut self, raw_handle: RawHandle, deadline: Instant) -> bool {
        if let Some(&idx) = self.handles.get(&raw_handle) {
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

    pub fn remove(&mut self, raw_handle: RawHandle) {
        if let Some(idx) = self.handles.remove(&raw_handle) {
            // NOTE: O(1) time, instead of `remove(idx)` which is O(n).
            let _ = self.buf.swap_remove(idx);

            if self.len() == 1 {
                debug_assert!(
                    self.handles.len() == 1,
                    "there should be one handle remaining for the last timer"
                );

                let entry = self
                    .handles
                    .iter_mut()
                    .next()
                    .expect("there should be one handle remaining for the last timer");

                *entry.1 = 0;
            } else {
                self.sift_down(idx, self.len());
            }
        }
    }

    pub const fn heap_iter(&mut self) -> HeapIter<'_> {
        HeapIter {
            heap: self,
            curr: 0,
        }
    }

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

            self.handles.insert(self.buf[pos].raw_handle, pos);
            self.handles.insert(self.buf[parent].raw_handle, parent);

            pos = parent;
        }

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

                self.handles.insert(self.buf[pos].raw_handle, pos);
                self.handles.insert(self.buf[min].raw_handle, min);

                pos = min;
            }
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
    use std::time::{Duration, Instant};

    #[test]
    fn test_push_one() {
        rt! {
            let mut heap = TimerHeap::new();

            let deadline1 = Instant::now() + Duration::from_secs(1);
            let _handle1 = heap.push(deadline1, Waker::noop().clone());

            assert!(!heap.is_empty());
            assert_eq!(heap.len(), 1);
            assert_eq!(heap.peek().unwrap().deadline, deadline1);
        }
    }

    #[test]
    fn test_pop_one() {
        rt! {
            let mut heap = TimerHeap::new();

            let deadline1 = Instant::now() + Duration::from_secs(1);
            let _handle1 = heap.push(deadline1, Waker::noop().clone());

            assert_eq!(heap.len(), 1);
            assert!(!heap.is_empty());
            assert_eq!(heap.peek().unwrap().deadline, deadline1);

            let popped = heap.pop();

            assert!(heap.is_empty());
            assert_eq!(heap.len(), 0);
            assert_eq!(popped.unwrap().deadline, deadline1);
        }
    }
}

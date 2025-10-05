/// A priority queue implemented with a binary min-heap.
///
/// Maintains the property where the smallest element is at the root, and every
/// parent node is smaller than or equal to its children.
///
/// # Time Complexity
///
/// | [push]  | [pop]         | [peek] |
/// |---------|---------------|--------|
/// | *O*(1)~ | *O*(log(*n*)) | *O*(1) |
///
/// [push]: MinHeap::push
/// [pop]:  MinHeap::pop
/// [peek]: MinHeap::peek
#[derive(Debug)]
pub(crate) struct MinHeap<T> {
    /// Internal buffer is `Vec` for cache locality and fast index-based access.
    buf: Vec<T>,
}

#[allow(dead_code)]
pub(crate) struct IntoIterSorted<T> {
    inner: MinHeap<T>,
}

impl<T: Ord> Iterator for IntoIterSorted<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.inner.len(), Some(self.inner.len()))
    }
}

/// Encodes the position of the item to sift from and the direction of sifting.
/// The upper 63 bits store the position, and the lowest bit indicates the
/// direction: `1` for upward (sift-up) and `0` for downward (sift-down).
#[repr(transparent)]
struct SiftInfo(u64);

impl SiftInfo {
    fn new(pos: usize, should_sift_up: bool) -> Self {
        // `usize` is platform-dependent (`u32` or `u64`), but converting to
        // `u64` ensures consistent bit-packing across architectures. We store
        // the sift direction in the most significant bit (MSB), which won't
        // interfere with valid position values, since Rust collections are
        // limited to `isize::MAX`, which fits within the lower 63 bits of a
        // `u64`.
        let packed = (pos as u64) | ((should_sift_up as u64) << 63);

        SiftInfo(packed)
    }

    fn pos(&self) -> usize {
        (self.0 & !(1 << 63)) as usize
    }

    fn should_sift_up(&self) -> bool {
        ((self.0 >> 63) & 0x1) != 0
    }

    #[allow(dead_code)]
    fn should_sift_down(&self) -> bool {
        ((self.0 >> 63) & 0x1) == 0
    }
}

/// Guard used to `heapify` the binary heap automatically on `Drop`.
///
/// https://doc.rust-lang.org/src/alloc/collections/binary_heap/mod.rs.html#484
struct HeapifyGuard<'a, T: Ord> {
    heap: &'a mut MinHeap<T>,
    sift_info: SiftInfo,
}

impl<T: Ord> Drop for HeapifyGuard<'_, T> {
    fn drop(&mut self) {
        let pos = self.sift_info.pos();

        if self.sift_info.should_sift_up() {
            debug_assert!(
                pos < self.heap.len(),
                "invalid `pos` provided when sifting up: {}",
                pos
            );

            // SAFETY: `pos` is < heap.len(), making the range valid.
            //
            // `sift_up` in range `0..=pos`.
            unsafe {
                self.heap.sift_up(0, pos);
            }
        } else {
            debug_assert!(
                pos <= self.heap.len(),
                "invalid `pos` provided when sifting up: {}",
                pos
            );

            // SAFETY: `pos` is <= heap.len(), making the range valid.
            //
            // `sift_down` in range `0..pos`.
            unsafe {
                self.heap.sift_down(pos, 0);
            }
        }
    }
}

impl<T: Ord> MinHeap<T> {
    /// Creates an empty `MinHeap`.
    #[inline]
    pub(crate) const fn new() -> Self {
        MinHeap { buf: vec![] }
    }

    /// Creates an empty `MinHeap` with at least the specified capacity.
    ///
    /// The binary heap will be able to hold at least capacity elements without
    /// reallocating. This method is allowed to allocate for more elements than
    /// `capacity`. If `capacity` is zero, the binary heap will not allocate.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        MinHeap {
            buf: Vec::with_capacity(capacity),
        }
    }

    /// Pushes an item onto the binary heap.
    pub(crate) fn push(&mut self, item: T) {
        let guard = HeapifyGuard {
            // The item to `sift_up` will be at this position.
            sift_info: SiftInfo::new(self.len(), true),
            heap: self,
        };

        // Appending `item` maintains the invariant of a complete binary tree,
        // meaning every level, except possibly the last, is fully filled.
        guard.heap.buf.push(item);

        // `HeapifyGuard` rebuilds the heap on drop...
    }

    /// Removes the smallest item from the binary heap and returns it, or `None`
    /// if it is empty.
    pub(crate) fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            let guard = HeapifyGuard {
                // The item to `sift_down` was at this position.
                sift_info: SiftInfo::new(self.len() - 1, false),
                heap: self,
            };

            // Removes the smallest element, replacing it with the last element
            // of the heap. Ensures the operation is *O*(1), instead of
            // `remove(0)` which shifts all elements after to the left (*O*(n)).
            Some(guard.heap.buf.swap_remove(0))

            // `HeapifyGuard` rebuilds the heap on drop...
        }
    }

    /// Returns a reference to the smallest item in the binary heap, or `None`
    /// if it is empty.
    #[allow(dead_code)]
    pub(crate) fn peek(&self) -> Option<&T> {
        self.buf.first()
    }

    /// Returns the length of the binary heap.
    #[inline]
    pub(crate) const fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns the number of elements the binary heap can hold without
    /// reallocating.
    #[inline]
    #[allow(dead_code)]
    pub(crate) const fn capacity(&self) -> usize {
        self.buf.capacity()
    }

    /// Returns `true` if the binary heap is empty.
    #[inline]
    pub(crate) const fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Returns an iterator which retrieves elements in heap order.
    ///
    /// This method consumes the original heap.
    #[inline]
    #[allow(dead_code)]
    pub(crate) const fn into_iter_sorted(self) -> IntoIterSorted<T> {
        IntoIterSorted { inner: self }
    }

    /// Restores the min-heap invariant by fixing any violations caused after
    /// an insertion.
    ///
    /// `start` specifies the upper bound (inclusive) for where the sifting
    /// should stop. `pos` is the index of the item that is being moved up.
    ///
    /// Returns the new position of the item.
    ///
    /// # Safety
    ///
    /// The range `start..=pos` must lie entirely within the bounds of the heap.
    /// This function may panic due to out-of-bounds access otherwise.
    unsafe fn sift_up(&mut self, start: usize, mut pos: usize) -> usize {
        // For an element at index `i`:
        //
        // - Left child: 2i + 1
        // - Right child: 2i + 2
        // - Parent: (i - 1) / 2
        while pos > start {
            let parent = (pos - 1) / 2;

            if self.buf[pos] >= self.buf[parent] {
                break;
            }

            // Bubble the item upward, swapping it with its parent.
            self.buf.swap(pos, parent);

            pos = parent;
        }

        pos
    }

    /// Restores the min-heap invariant by fixing any violations caused after
    /// a removal.
    ///
    /// `end` specifies the upper bound (exclusive) for where the sifting
    /// should stop. `pos` is the index of the item that is being moved down.
    ///
    /// Returns the new position of the item.
    ///
    /// # Safety
    ///
    /// The range `pos..end` must lie entirely within the bounds of the heap.
    /// This function may panic due to out-of-bounds access otherwise.
    unsafe fn sift_down(&mut self, end: usize, mut pos: usize) -> usize {
        // For an element at index `i`:
        //
        // - Left child: 2i + 1
        // - Right child: 2i + 2
        // - Parent: (i - 1) / 2
        loop {
            let left = 2 * pos + 1;
            let right = 2 * pos + 2;

            // Comparison starts with the left child. Once the left child index
            // exceeds or equals `end`, we stop comparisons.
            if left >= end {
                break;
            }

            let mut min = pos;

            if self.buf[pos] >= self.buf[left] {
                min = left;
            }

            // Check if the right child exists before comparing. The `&&` is
            // short-circuiting, so the second condition won't run if `right` is
            // out of bounds.
            if right < end && self.buf[min] >= self.buf[right] {
                min = right;
            }

            // Check if a smaller child item was encountered.
            if min != pos {
                self.buf.swap(min, pos);
                pos = min;
            } else {
                // Can no longer sift down the item.
                break;
            }
        }

        pos
    }
}

impl<T: Ord> Default for MinHeap<T> {
    /// Creates an empty `MinHeap`.
    #[inline]
    fn default() -> Self {
        MinHeap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let mut heap: MinHeap<i32> = MinHeap::new();
        assert!(heap.peek().is_none());
        assert!(heap.pop().is_none());
        assert_eq!(heap.len(), 0);
        assert!(heap.is_empty());
    }

    #[test]
    fn test_push_and_peek() {
        let mut heap = MinHeap::new();
        heap.push(10);
        assert_eq!(heap.peek(), Some(&10));
        heap.push(5);
        assert_eq!(heap.peek(), Some(&5));
        heap.push(15);
        assert_eq!(heap.peek(), Some(&5));
    }

    #[test]
    fn test_pop() {
        let mut heap = MinHeap::new();
        let mut values = vec![12, 3, 25, 7, 9, 1];

        for &v in &values {
            heap.push(v);
        }

        values.sort();

        for &v in &values {
            assert_eq!(heap.pop(), Some(v));
        }

        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn test_duplicates() {
        let mut heap = MinHeap::new();

        heap.push(7);
        heap.push(7);
        heap.push(3);
        heap.push(3);
        heap.push(5);
        heap.push(5);

        assert_eq!(heap.peek(), Some(&3));

        let sorted: Vec<_> = heap.into_iter_sorted().collect();
        assert_eq!(sorted, vec![3, 3, 5, 5, 7, 7]);
    }
}

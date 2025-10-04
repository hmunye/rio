use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::task::{Context, Poll};
use std::thread;

use crate::rt::task::{Id, TaskHandle};
use crate::rt::waker::TaskWaker;

/// Single-threaded `Task` scheduler.
#[derive(Debug)]
pub(crate) struct Scheduler {
    /// Stores all live tasks keyed by their ID, each paired with its
    /// `TaskWaker`. Enables efficient `O(log n)` lookup and maintains order if
    /// needed (e.g., for fair scheduling or debugging).
    #[allow(dead_code)]
    tasks: BTreeMap<Id, (TaskHandle, TaskWaker)>,
    /// Queue of task IDs that are ready to be polled. Storing only IDs keeps
    /// the queue lightweight and avoids cloning or holding multiple `Task`
    /// handles. Wrapped in `RefCell` for interior mutability since components
    /// like `Waker`s only have shared access to the `Scheduler`.
    #[allow(dead_code)]
    ready: RefCell<VecDeque<Id>>,
}

impl Scheduler {
    /// Creates a new `Scheduler` instance.
    #[inline]
    pub(crate) fn new() -> Self {
        Scheduler {
            tasks: Default::default(),
            ready: RefCell::new(Default::default()),
        }
    }

    /// Schedules the given `TaskHandle` using the provided `TaskWaker`,
    /// blocking the current thread until the underlying future resolves.
    pub(crate) fn block_on(&self, task: TaskHandle, waker: TaskWaker) {
        let mut ctx = Context::from_waker(&waker);

        loop {
            match task.borrow_mut().poll(&mut ctx) {
                Poll::Ready(_) => break,
                Poll::Pending => thread::yield_now(),
            }
        }
    }
}

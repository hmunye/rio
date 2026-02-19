use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::task::{Context, Waker};
use std::time::Instant;

#[cfg(feature = "io")]
use crate::rt::io::Driver;
#[cfg(feature = "io")]
use std::os::unix::io::RawFd;

use crate::rt::task::{TaskHandle, TaskId, TaskWaker};
use crate::rt::timer::TimerEntry;
use crate::rt::util::MinHeap;

type TaskEntry = (TaskHandle, TaskWaker);

/// Single-threaded `scheduler` and `executor`.
#[derive(Debug)]
pub struct Scheduler {
    /// Stores all live tasks keyed by their ID and paired with a [`TaskWaker`].
    tasks: RefCell<HashMap<TaskId, TaskEntry>>,
    /// Queue of task IDs ready to be polled. Storing only IDs keeps the queue
    /// lightweight and avoids cloning or holding multiple task handles.
    ready: RefCell<VecDeque<TaskId>>,
    /// Holds tasks that are spawned while the scheduler is polling tasks,
    /// preventing reentrant mutable borrows. Each `tick` borrows the scheduler
    /// fields mutably, but spawning also requires a mutable borrow. To avoid
    /// double-borrowing during polling, newly spawned tasks are temporarily
    /// stored here and later transferred on each `tick`.
    pending: RefCell<Vec<TaskEntry>>,
    /// A priority queue of timers associated with tasks, keyed by their
    /// scheduled wake-up time.     
    timers: RefCell<MinHeap<TimerEntry>>,
    /// Handles registering and waiting on I/O events, waking tasks when file
    /// descriptors become ready.
    #[cfg(feature = "io")]
    io: RefCell<Driver>,
}

impl Scheduler {
    /// Creates a new `Scheduler`.
    pub(crate) fn new() -> Self {
        Scheduler {
            tasks: RefCell::default(),
            ready: RefCell::default(),
            pending: RefCell::default(),
            timers: RefCell::default(),

            #[cfg(feature = "io")]
            io: RefCell::new(Driver::new()),
        }
    }

    /// Schedules the given [`TaskHandle`] and associated [`TaskWaker`],
    /// blocking the current thread until the scheduler enters an idle state.
    ///
    /// This idle state indicates there are no more active or pending tasks
    /// to be polled.
    pub(crate) fn block_on_task(&self, task: TaskHandle, waker: TaskWaker) {
        let id = task.borrow().id;

        self.schedule_task(id);
        self.tasks.borrow_mut().insert(id, (task, waker));

        while !self.is_idle() {
            #[cfg(feature = "io")]
            {
                // Use the closest expiring timer as the `timeout` for the
                // driver, with a fallback of `-1` indicating the I/O driver
                // should block.
                let timeout = self
                    .timers
                    .borrow()
                    .peek()
                    .and_then(|entry| entry.deadline.checked_duration_since(Instant::now()))
                    .map(|duration| duration.as_millis() as i32)
                    .unwrap_or(-1);

                self.io.borrow_mut().poll(timeout);
            }
            self.tick();
        }
    }

    /// Returns `true` if the scheduler has no remaining tasks to poll
    /// (e.g, no currently active tasks and no spawned tasks).
    #[inline]
    fn is_idle(&self) -> bool {
        self.tasks.borrow().is_empty() && self.pending.borrow().is_empty()
    }

    /// Marks the task associated with the provided ID as ready to be polled.
    #[inline]
    pub(crate) fn schedule_task(&self, id: TaskId) {
        self.ready.borrow_mut().push_back(id);
    }

    /// Schedules the given [`TaskHandle`] and associated [`TaskWaker`],
    /// polling it concurrently with other tasks.
    pub(crate) fn spawn_task(&self, task: TaskHandle, waker: TaskWaker) {
        self.pending.borrow_mut().push((task, waker));
    }

    /// Registers a timer with the scheduler, associating it with the provided
    /// [`Waker`].
    pub(crate) fn register_timer(&self, duration: Instant, waker: Waker) {
        self.timers.borrow_mut().push(TimerEntry {
            deadline: duration,
            waker,
        });
    }

    /// Registers the given file descriptor with the I/O driver, associating it
    /// with the provided [`Waker`].
    #[cfg(feature = "io")]
    pub(crate) fn register_fd(&self, fd: RawFd, events: u32, waker: Waker) {
        self.io.borrow_mut().register(fd, events, waker);
    }

    /// Change the settings associated with the given file descriptor to the new
    /// settings specified in `events`. This function should be called on a
    /// file descriptor that is already registered.
    #[cfg(feature = "io")]
    pub(crate) fn modify_fd(&self, fd: RawFd, events: u32) {
        self.io.borrow_mut().modify(fd, events);
    }

    /// Unregisters the given file descriptor with the I/O driver.
    #[cfg(feature = "io")]
    pub(crate) fn unregister_fd(&self, fd: RawFd) {
        self.io.borrow_mut().unregister(fd);
    }

    /// Polls all tasks on the `ready` queue, processing any pending spawned
    /// tasks and timers that may exist.
    fn tick(&self) {
        // These should be processed first so that the `ready` queue correctly
        // reflects all tasks that can make progress.
        self.process_pending();
        self.process_timers();

        while let Some(id) = self.ready.borrow_mut().pop_front() {
            // Temporarily remove the task entry from the map.
            let entry = self.tasks.borrow_mut().remove(&id);
            let Some((task, waker)) = entry else {
                continue;
            };

            // Mark task as not currently scheduled.
            task.borrow().scheduled.set(false);

            let mut ctx = Context::from_waker(&waker);
            let poll = {
                let mut task_ref = task.borrow_mut();
                task_ref.poll(&mut ctx)
            };

            if poll.is_pending() {
                // Re-insert the entry for future polling.
                self.tasks.borrow_mut().insert(id, (task, waker));
            }

            // Drop `task` and `waker` if polling resolved...
        }
    }

    /// Process all pending spawned tasks, queuing them to be polled on the next
    /// `tick`.
    fn process_pending(&self) {
        let mut pending = self.pending.borrow_mut();

        for (task, waker) in pending.drain(..) {
            let id = task.borrow().id;

            self.schedule_task(id);
            self.tasks.borrow_mut().insert(id, (task, waker));
        }
    }

    /// Process all expired timers, waking associated tasks with paired waker.
    ///
    /// Timers are processed in order of their scheduled wake-up time.
    fn process_timers(&self) {
        let time_now = Instant::now();

        loop {
            let entry = self.timers.borrow_mut().pop();
            let Some(entry) = entry else {
                // No timers have been registered.
                break;
            };

            if entry.deadline <= time_now {
                entry.waker.wake();
            } else {
                self.timers.borrow_mut().push(entry);
                // Since the earliest timeout in the heap hasn't expired, all
                // other timers are guaranteed not to have expired either, so
                // stop processing.
                break;
            }
        }
    }
}

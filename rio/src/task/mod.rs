//! Asynchronous Tasks.
//!
//! Tasks are the fundamental units of non-blocking execution within the `rio`
//! runtime. Often referred to as __green threads__, they operate cooperatively,
//! able to yield control back to the scheduler to allow other ready tasks to
//! make progress.
//!
//! Tasks __must never block__ the current thread, and should always utilize the
//! provided asynchronous primitives or opt-in to [`cooperative scheduling`].
//!
//! ### Task Lifecycle
//!
//! ```text
//!                +---------------------------------+
//!                |            Scheduled            |
//!                |                                 |
//!                | - Entered on rio::spawn() or    |
//!                |   Runtime::block_on()           |
//!                | - Task registered with the      |
//!                |   scheduler                     |
//!                | - Queued for the current "tick" |
//!                |   (scheduler cycle)             |
//!                | - Awaiting first Task::poll()   |
//!                +---------------------------------+
//!                                |
//!                                v
//!                +---------------------------------+
//!                |             Running             |
//!  +-----------> |                                 |
//!  |             | - Entered on Task::poll()       |
//!  |             | - Inner Future is being polled  |
//!  |             | - May be deferred to next       |
//!  |             |   "tick" on yield_now() or      |
//!  |             |   cooperative budget exhaustion |
//!  |             +---------------------------------+
//!  |                    |                       |
//!  |   `Poll::Pending`  |                       |  `Poll::Ready`
//!  |                    |                       |  
//!  |                    v                       v
//!  |      +--------------------------+   /-----------------------------------\
//!  |      |           Idle           |   |  Completion Handling (transition) |
//!  |      |                          |   |                                   |
//!  |      | - Remains registered     |   | - Receives Future output or       |
//!  |      |   with the scheduler     |   |   _panic_ payload                 |
//!  |      | - Not re-queued (unless  |   | - Wakes any awaiting JoinHandle   |
//!  |      |   task was deferred)     |   \-----------------------------------/
//!  |      | - Waiting for external   |           /         |        \
//!  |      |   wake                   |          /          |         \
//!  |      +--------------------------+         /           |          \
//!  |         |                                /            |           \
//!  +---------+                               /             |            \
//!       ^                                   /              |             \
//!       |                                  /               |              \
//!       |                                 /                |               \
//!  wake to reschedule (I/O, timer, etc.) /                 |                \
//!                                       v                  v                 v
//!                    +--------------------+   +------------------------+   +-----------------------+
//!                    |      Finished      |   |        Consumed        |   |         Panic         |
//!                    |                    |   |                        |   |                       |
//!                    | - Task completed   |   | - Output taken by      |   | - Task panicked       |
//!                    |   successfully     |   |   JoinHandle or handle |   | - JoinHandle resolves |
//!                    | - Output available |   |   was dropped          |   |   to JoinError        |
//!                    |   for JoinHandle   |   |                        |   |                       |
//!                    +--------------------+   +------------------------+   +-----------------------+
//! ```
//!
//! #### Cancellation
//!
//! Tasks can be canceled via [`JoinHandle::cancel()`]. This transitions the
//! task into the `Canceled` stage, after which it will be removed on the next
//! "poll" attempt by the scheduler.
//!
//! [`cooperative scheduling`]: coop
//! [`JoinHandle::cancel()`]: JoinHandle::cancel

pub mod coop;

mod id;
pub use id::{Id, id};

mod spawn;
pub use spawn::spawn;

mod yield_now;
pub use yield_now::yield_now;

mod join;
pub use join::{JoinError, JoinHandle};

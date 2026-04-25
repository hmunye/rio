//! Asynchronous Tasks.
//!
//! Tasks are the fundamental units of non-blocking execution within the `rio`
//! runtime. Often referred to as __green threads__, they operate cooperatively,
//! able to yield control back to the scheduler to allow other ready tasks to
//! progress.
//!
//! Tasks __must never block__ the current thread. Always utilize the provided
//! non-blocking asynchronous primitives or opt-in to [`cooperative scheduling`]
//! to ensure the runtime remains responsive.
//!
//! ### Task Lifecycle
//!
//! ```text
//!                +---------------------------------+
//!                |            Scheduled            |
//!                |                                 |
//!                | - Entered on `rio::spawn()` or  |
//!                |   `Runtime::block_on()`         |
//!                | - Task registered with the      |
//!                |   scheduler                     |
//!                | - Queued for the current "tick" |
//!                |   (scheduler cycle)             |
//!                | - Awaiting first `Task::poll()` |
//!                +---------------------------------+
//!                                |
//!                                v
//!                +---------------------------------+
//!                |             Running             |
//!  +-----------> |                                 |
//!  |             | - Entered on `Task::poll()`     |
//!  |             | - Inner `Future` is being       |
//!  |             |   polled                        |
//!  |             | - May be deferred to next       |
//!  |             |   "tick" on `yield_now()` or    |
//!  |             |   cooperative budget exhaustion |
//!  |             +---------------------------------+
//!  |                    |                       |
//!  |   `Poll::Pending`  |                       |  `Poll::Ready`
//!  |                    |                       |  
//!  |                    v                       v
//!  |      +--------------------------+   /-----------------------------------\
//!  |      |           Idle           |   |     Callback Execution (Action)   |
//!  |      |                          |   |                                   |
//!  |      | - Remains registered     |   | - Receives `Future` output or     |
//!  |      |   with the scheduler     |   |   panic payload                   |
//!  |      | - Not re-queued (unless  |   | - Access to `Weak<TaskState>`     |
//!  |      |   task was deferred)     |   | - Wakes the `JoinHandle` awaiting |
//!  |      | - Waiting for external   |   |   its completion, if one exists   |
//!  |      |   wake via `Waker`       |   \-----------------------------------/
//!  |      +--------------------------+      /           |          \
//!  |         |                             /            |           \
//!  +---------+                            /             |            \
//!       ^                                /              |             \
//!       |                               /               |              \
//!       |                              /                |               \
//!  wake (I/O, timer, etc.)            /                 |                \
//!                                    v                  v                 v
//!                 +--------------------+   +------------------------+   +-----------------------+
//!                 |      Finished      |   |        Consumed        |   |         Panic         |
//!                 |                    |   |                        |   |                       |
//!                 | - Task completed   |   | - Output taken by      |   | - Task panicked       |
//!                 |   successfully     |   |   `JoinHandle` or      |   | - JoinHandle resolves |
//!                 | - Output available |   |   handle was dropped   |   |   to `JoinError`      |
//!                 |   for `JoinHandle` |   |                        |   |                       |
//!                 +--------------------+   +------------------------+   +-----------------------+
//! ```
//!
//! #### External Lifecycle Control
//!
//! | API                                                  | Effect on Task(s)                                                                                                          |
//! |------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------|
//! | [`yield_now()`]                                      | __Running__ -> __Idle__; task will be polled on the _next_ tick.                                                |
//! | [`JoinHandle::cancel()`]                             | __Current Stage__ -> __Canceled__; task removed the next time polling is attempted.                             |
//! | [`coop::poll_proceed()`], [`coop::consume_budget()`] | __Running__ -> __Idle__ if the current execution budget is exhausted; task polled on the _next_ tick if `true`. |
//! | [`rt::shutdown()`]                                   | Signals the runtime to exit after the *root* task completes; does __not__ wait for other spawned tasks.         |
//!
//!
//! [`cooperative scheduling`]: coop
//! [`JoinHandle::cancel()`]: JoinHandle::cancel
//! [`coop::poll_proceed()`]: coop::poll_proceed
//! [`coop::consume_budget()`]: coop::consume_budget
//! [`rt::shutdown()`]: crate::rt::shutdown

pub mod coop;

mod id;
pub use id::{Id, id};

mod spawn;
pub use spawn::spawn;

mod yield_now;
pub use yield_now::yield_now;

mod join;
pub use join::{JoinError, JoinHandle};

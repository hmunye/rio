//! Cooperative Scheduling Utilities.
//!
//! ## Why Cooperation Matters
//!
//! `rio` relies on cooperative scheduling to manage multiple tasks on a single
//! thread. Unlike OS threads, the runtime cannot forcibly interrupt a running
//! task. If a task's [`poll`] method executes heavy computation without
//! yielding, it monopolizes the scheduler, preventing other tasks from making
//! progress. This leads to __starvation__ of ready tasks and delays in
//! time-bound resources (i.e., timers).
//!
//! ## Cooperative Scheduling
//!
//! Long-running CPU-intensive workloads can stall the entire runtime.
//!
//! ```
//! use std::future;
//!
//! // Problem: This future __starves__ other tasks until complete.
//! async fn starving_fut() {
//!     for _ in 0..1_000_000_000 {
//!         /* doing work ... */
//!
//!         // Does _not_ yield to the runtime, since it is always ready.
//!         future::ready(()).await;
//!     }
//! }
//! ```
//!
//! To ensure fair scheduling, __wrap CPU-intensive futures__ with this module's
//! primitives so they periodically yield back to the `rio` runtime.
//!
//! ```
//! use std::future;
//!
//! // Solution: Use `make_cooperative` to prevent __starvation__.
//! async fn blocking_loop() {
//!     for _ in 0..1_000_000_000 {
//!         /* doing work ... */
//!
//!         // Ensures this future participates in the runtime’s cooperative
//!         // execution, allowing other tasks to make progress after the budget
//!         // is exhausted.
//!         rio::task::coop::make_cooperative(future::ready(())).await;
//!     }
//! }
//! ```
//!
//! For work within tasks that should bypass cooperative checks, see
//! [`make_unconstrained`].
//!
//! [`poll`]: std::future::Future::poll

mod budget;
pub use budget::has_budget_remaining;
pub(crate) use budget::{Budget, with_initial, with_unconstrained};

mod proceed;
pub use proceed::{BudgetGuard, poll_proceed};

mod consume;
pub use consume::consume_budget;

mod cooperative;
pub use cooperative::{Cooperative, make_cooperative};

mod unconstrained;
pub use unconstrained::{Unconstrained, make_unconstrained};

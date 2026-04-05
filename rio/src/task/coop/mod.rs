//! Cooperative Scheduling Utilities.
//!
//! ### Why Cooperation Matters
//!
//! Unlike OS threads, which can be forcibly interrupted by the OS kernel
//! (preemption), `rio` relies on __cooperative scheduling__. This means the
//! runtime cannot interrupt a task; a task must voluntarily yield control.
//!
//! If a task performs heavy CPU-bound computation within its [`poll`] method
//! without returning [`Poll::Pending`], it monopolizes the underlying thread.
//! This leads to __task starvation__, where other ready tasks are prevented
//! from making progress.
//!
//! ### Cooperative Scheduling
//!
//! A long-running loop that performs significant work without yielding can
//! stall the entire runtime:
//!
//! ```
//! use std::future;
//!
//! async fn starving_fut() {
//!     for _ in 0..1_000_000_000 {
//!         // ...
//!
//!         // Even though we `await` here, because the future is always ready,
//!         // the runtime immediately polls this task again.
//!         future::ready(()).await;
//!     }
//! }
//! ```
//!
//! To ensure fair scheduling, make use of the cooperative utilities from this
//! module. These utilities ensure that the task periodically yields control, in
//! a way that allows other tasks to run:
//!
//! ```
//! use std::future;
//!
//! async fn blocking_loop() {
//!     for _ in 0..1_000_000_000 {
//!         // ...
//!
//!         // This ensures the future participates in the runtime’s cooperative
//!         // scheduling, counting towards the task's execution budget.
//!         rio::task::coop::make_cooperative(future::ready(())).await;
//!     }
//! }
//! ```
//!
//! For futures within tasks that should bypass cooperative checks, see
//! [`make_unconstrained`].
//!
//! [`poll`]: Future::poll
//! [`Poll::Pending`]: std::task::Poll::Pending

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

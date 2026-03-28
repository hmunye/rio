//! Asynchronous Tasks
//!
//! A task is a lightweight, non‑blocking unit of execution (**green thread**)
//! scheduled by the `rio` runtime. Tasks run cooperatively until they _yield_,
//! after which the runtime can poll other ready tasks.
//!
//! Due to the scheduler being single‑threaded, a task should **not block** the
//! current thread.

mod id;
pub use id::{Id, id};

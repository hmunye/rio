//! Lightweight, non-blocking units of execution, similar to OS threads, but
//! rather than being managed by the OS scheduler, they are managed by the
//! [runtime].
//!
//! [runtime]: crate::rt

mod core;
pub(crate) use core::{Task, TaskHandle, TaskId};

mod waker;
pub(crate) use waker::TaskWaker;

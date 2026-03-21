use std::cell::{Cell, RefCell};

use crate::runtime::{scheduler, task};

struct Context {
    /// Runtime handle associated with the current thread.
    current: RefCell<Option<scheduler::Handle>>,
    /// ID of the "active" task on the current thread.
    current_task_id: Cell<Option<task::Id>>,
}

thread_local! {
    static CONTEXT: Context = const {
        Context {
            current: RefCell::new(None),
            current_task_id: Cell::new(None),
        }
    }
}

/// Associates the given runtime handle with the current thread.
///
/// # Panics
///
/// Panics if the current thread is already associated with a runtime handle.
#[inline]
pub fn set_current(handle: &scheduler::Handle) {
    CONTEXT.with(|ctx| {
        let mut current = ctx.current.borrow_mut();
        assert!(
            current.is_none(),
            "runtime context already associated with the current thread"
        );
        *current = Some(handle.clone());
    });
}

/// Removes the runtime handle associated with the current thread.
#[inline]
pub fn drop_current() {
    CONTEXT.with(|ctx| ctx.current.take());
}

/// Executes the provided closure using the runtime handle of the current
/// thread.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
#[inline]
pub fn with_current<R, F: FnOnce(&scheduler::Handle) -> R>(f: F) -> R {
    CONTEXT
        .with(|ctx| ctx.current.borrow().as_ref().map(f))
        .expect("no runtime context associated with the current thread")
}

/// Returns the ID of the "active" task on the current thread, if any.
#[inline]
pub fn current_task_id() -> Option<task::Id> {
    CONTEXT
        .try_with(|ctx| ctx.current_task_id.get())
        .unwrap_or(None)
}

/// Sets the "active" task ID for the current thread, returning the previous
/// task ID.
#[inline]
pub fn set_current_task_id(id: Option<task::Id>) -> Option<task::Id> {
    CONTEXT
        .try_with(|ctx| ctx.current_task_id.replace(id))
        .unwrap_or(None)
}

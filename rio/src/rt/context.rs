use std::cell::{Cell, RefCell};

use crate::{rt, task};

struct Context {
    /// Runtime handle associated with the current thread.
    handle: RefCell<Option<rt::Handle>>,
    /// [`Id`] of the currently running task on the current thread.
    ///
    /// [`Id`]: task::Id
    task_id: Cell<Option<task::Id>>,
}

thread_local! {
    static CONTEXT: Context = const {
        Context {
            handle: RefCell::new(None),
            task_id: Cell::new(None),
        }
    }
}

/// Sets the provided runtime handle for the current thread.
///
/// # Panics
///
/// Panics if the current thread is already associated with a runtime handle.
#[inline]
pub fn set_current(handle: &rt::Handle) {
    CONTEXT.with(|ctx| {
        assert!(
            ctx.handle.replace(Some(handle.clone())).is_none(),
            "runtime context already associated with the current thread"
        );
    });
}

/// Removes the runtime handle associated with the current thread.
#[inline]
pub fn drop_current() {
    CONTEXT.with(|ctx| ctx.handle.take());
}

/// Returns the `Id` of the currently running task on the current thread.
#[inline]
pub fn current_task() -> Option<task::Id> {
    CONTEXT.with(|ctx| ctx.task_id.get())
}

/// Sets the `Id` of the currently running task on the current thread, returning
/// the previous `Id`.
#[inline]
pub fn set_current_task(id: Option<task::Id>) -> Option<task::Id> {
    CONTEXT.with(|ctx| ctx.task_id.replace(id))
}

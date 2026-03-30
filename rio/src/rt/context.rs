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

/// Executes the provided closure using the runtime handle of the current
/// thread.
///
/// # Panics
///
/// Panics if the current thread is not associated with a runtime handle.
#[inline]
pub fn with_handle<R, F: FnOnce(&rt::Handle) -> R>(f: F) -> R {
    CONTEXT
        .with(|ctx| ctx.handle.borrow().as_ref().map(f))
        .expect("no runtime context associated with the current thread")
}

/// Sets the provided runtime handle for the current thread.
///
/// # Panics
///
/// Panics if the current thread is already associated with a runtime handle.
#[inline]
pub fn set_handle(handle: &rt::Handle) {
    CONTEXT.with(|ctx| {
        assert!(
            ctx.handle.replace(Some(handle.clone())).is_none(),
            "runtime context already associated with the current thread"
        );
    });
}

/// Removes the runtime handle associated with the current thread.
#[inline]
pub fn drop_handle() {
    CONTEXT.with(|ctx| ctx.handle.take());
}

/// Returns the `Id` of the currently running task on the current thread.
#[inline]
pub fn task_id() -> Option<task::Id> {
    CONTEXT.with(|ctx| ctx.task_id.get())
}

/// Sets the `Id` of the currently running task on the current thread, returning
/// the previous `Id`.
#[inline]
pub fn set_task_id(id: Option<task::Id>) -> Option<task::Id> {
    CONTEXT.with(|ctx| ctx.task_id.replace(id))
}

use std::cell::{Cell, RefCell};

use crate::runtime::{scheduler, task};

struct Context {
    /// Runtime handle associated with the current thread.
    current: RefCell<Option<scheduler::Handle>>,
    /// ID of the currently active task on the current thread, if any.
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
/// Panics if there is an active runtime context on the current thread.
#[inline]
pub fn set_current(handle: &scheduler::Handle) {
    CONTEXT.with(|ctx| {
        let mut current = ctx.current.borrow_mut();
        assert!(
            !current.is_some(),
            "runtime context already associated with the current thread"
        );
        *current = Some(handle.clone());
    });
}

/// Removes the runtime handle associated with the current thread.
#[inline]
pub fn unset_current() {
    CONTEXT.with(|ctx| ctx.current.borrow_mut().take());
}

/// Returns the task ID currently active on this thread, if any.
#[inline]
#[allow(unused)]
pub fn current_task_id() -> Option<task::Id> {
    CONTEXT
        .try_with(|ctx| ctx.current_task_id.get())
        .unwrap_or(None)
}

/// Sets the active task ID for the current thread, returning the previous ID if
/// present.
#[inline]
#[allow(unused)]
pub fn set_current_task_id(id: Option<task::Id>) -> Option<task::Id> {
    CONTEXT
        .try_with(|ctx| ctx.current_task_id.replace(id))
        .unwrap_or(None)
}

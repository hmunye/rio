use std::cell::{Cell, RefCell};

use crate::rt;
use crate::task::{self, coop::Budget};

pub struct Snapshot {
    last_budget: Budget,
}

impl Snapshot {
    /// Returns how much execution budget has been consumed since the snapshot
    /// was last updated, or `0` if either the snapshot budget or the provided
    /// `budget` is _unconstrained_.
    pub fn used_since(&self) -> u8 {
        with_budget(|b| {
            match (self.last_budget.val(), b.get().val()) {
                (Some(snap), Some(curr)) => snap.saturating_sub(curr),
                _ => 0, // unconstrained: nothing _used_.
            }
        })
    }
}

struct Context {
    /// Runtime handle associated with the current thread.
    handle: RefCell<Option<rt::Handle>>,
    /// [`Id`] of the currently running task on the current thread.
    ///
    /// [`Id`]: task::Id
    task_id: Cell<Option<task::Id>>,
    /// Tracks the remaining execution budget for the current "tick", before
    /// tasks need to yield control to the runtime.
    budget: Cell<Budget>,
    /// Per-task snapshot of execution context on the current thread.
    snapshot: RefCell<Snapshot>,
}

thread_local! {
    static CONTEXT: Context = const {
        Context {
            handle: RefCell::new(None),
            task_id: Cell::new(None),
            budget: Cell::new(Budget::unconstrained()),
            snapshot: RefCell::new(Snapshot {
                last_budget: Budget::unconstrained()
            })
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
        .with(|cx| cx.handle.borrow().as_ref().map(f))
        .expect("no runtime context associated with the current thread")
}

/// Sets the provided runtime handle for the current thread.
///
/// # Panics
///
/// Panics if the current thread is already associated with a runtime handle.
#[inline]
pub fn set_handle(handle: &rt::Handle) {
    CONTEXT.with(|cx| {
        assert!(
            cx.handle.replace(Some(handle.clone())).is_none(),
            "runtime context already associated with the current thread"
        );
    });
}

/// Removes the runtime handle associated with the current thread.
#[inline]
pub fn drop_handle() {
    CONTEXT.with(|cx| cx.handle.take());
}

/// Returns the `Id` of the currently running task on the current thread.
#[inline]
pub fn task_id() -> Option<task::Id> {
    CONTEXT.with(|cx| cx.task_id.get())
}

/// Sets the `Id` of the currently running task on the current thread, returning
/// the previous `Id`.
#[inline]
pub fn set_task_id(id: Option<task::Id>) -> Option<task::Id> {
    CONTEXT.with(|cx| cx.task_id.replace(id))
}

/// Executes the provided closure using the `Budget` of the current thread.
#[inline]
pub fn with_budget<R, F: FnOnce(&Cell<Budget>) -> R>(f: F) -> R {
    CONTEXT.with(|cx| f(&cx.budget))
}

/// Sets the `Budget` of the current thread, returning the previous `Budget`.
#[inline]
pub fn set_budget(budget: Budget) -> Budget {
    CONTEXT.with(|cx| cx.budget.replace(budget))
}

/// Executes the provided closure using the `Snapshot` of the current thread.
#[inline]
pub fn with_snapshot<R, F: FnOnce(&Snapshot) -> R>(f: F) -> R {
    CONTEXT.with(|cx| f(&cx.snapshot.borrow()))
}

/// Updates the `Snapshot` of the current thread.
#[inline]
pub fn update_snapshot() {
    CONTEXT.with(|cx| {
        cx.snapshot.borrow_mut().last_budget = cx.budget.get();
    });
}

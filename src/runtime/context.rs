use std::cell::RefCell;

use crate::runtime::scheduler;

struct Context {
    /// Handle to the runtime scheduler on the current thread.
    current: RefCell<Option<scheduler::Handle>>,
}

thread_local! {
    static CONTEXT: Context = const {
        Context {
            current: RefCell::new(None),
        }
    }
}

/// Sets the scheduler for the runtime context on the current thread.
#[inline]
pub fn set_current(handle: &scheduler::Handle) {
    let _ = CONTEXT.with(|ctx| ctx.current.borrow_mut().replace(handle.clone()));
}

/// Clears the scheduler for the runtime context on the current thread.
#[inline]
pub fn unset_current() {
    let _ = CONTEXT.with(|ctx| ctx.current.borrow_mut().take());
}

use crate::rt::context;

/// Returns `true` if there is execution budget left for the current task to
/// proceed.
#[inline]
#[must_use]
pub fn has_budget_remaining() -> bool {
    context::with_budget(|b| b.get().has_remaining())
}

/// Execution Budget.
///
/// Tracks the number of polling iterations that may be performed within a
/// "tick", before control is yielded to the runtime.
#[derive(Debug, Clone, Copy)]
pub struct Budget(Option<u8>);

impl Budget {
    // One less than `tokio`'s initial value to fit within a `u128` bitmap.
    //
    // <https://docs.rs/tokio/latest/src/tokio/task/coop/mod.rs.html#116>
    pub const INITIAL: u8 = 127;

    #[must_use]
    pub const fn initial() -> Self {
        Budget(Some(Budget::INITIAL))
    }

    #[must_use]
    pub const fn unconstrained() -> Self {
        Budget(None)
    }

    /// Decrements the execution budget, returning `true` if the budget was not
    /// _exhausted_.
    #[must_use]
    pub fn consume_unit(&mut self) -> bool {
        self.0.is_none_or(|b| {
            let remaining = b > 0;
            self.0 = Some(b.saturating_sub(1));

            remaining
        })
    }

    /// Consumes `self`, returning its numeric value.
    pub const fn val(self) -> Option<u8> {
        self.0
    }

    /// Returns `true` if the budget is _unconstrained_.
    pub const fn is_unconstrained(self) -> bool {
        self.0.is_none()
    }

    fn has_remaining(self) -> bool {
        self.0.is_none_or(|b| b > 0)
    }
}

/// Executes the given closure within an _initial_ execution budget context.
pub fn with_initial<R>(f: impl FnOnce() -> R) -> R {
    with_budget(Budget::initial(), f)
}

/// Executes the given closure within an _unconstrained_ (unlimited) execution
/// budget context.
pub fn with_unconstrained<R>(f: impl FnOnce() -> R) -> R {
    with_budget(Budget::unconstrained(), f)
}

fn with_budget<R>(budget: Budget, f: impl FnOnce() -> R) -> R {
    struct ResetGuard {
        prev: Budget,
    }

    impl Drop for ResetGuard {
        fn drop(&mut self) {
            let _ = context::set_budget(self.prev);
        }
    }

    let _guard = ResetGuard {
        prev: context::set_budget(budget),
    };

    f()
}

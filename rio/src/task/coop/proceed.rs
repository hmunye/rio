use std::cell::Cell;
use std::task::Poll;

use crate::rt::context;
use crate::task::{self, coop::Budget};

/// Guard returned by [`poll_proceed`].
#[derive(Debug)]
#[must_use = "not using this guard will leave any consumed budget uncommitted"]
pub struct BudgetGuard(Cell<Budget>);

impl BudgetGuard {
    /// Signals that the current task has made progress, ensuring the execution
    /// budget is __not__ rolled back to its previous value.
    ///
    /// This should be called before a future returns `Poll::Ready` to indicate
    /// actual work was done.
    #[inline]
    pub fn made_progress(&self) {
        self.0.set(Budget::unconstrained());
    }
}

impl Drop for BudgetGuard {
    fn drop(&mut self) {
        let budget = self.0.get();

        if !budget.is_unconstrained() {
            let _ = context::set_budget(budget);
        }
    }
}

/// Decrements the current execution budget, returning a [`BudgetGuard`] if
/// allowed to proceed. Returns [`Poll::Pending`] if the budget is exhausted,
/// yielding control to the runtime.
///
/// The budget is restored to its state prior to calling `poll_proceed` when the
/// guard is dropped, unless committed using [`BudgetGuard::made_progress`]. It
/// is the caller's responsibility to do so when it _was_ able to make progress.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```no_run
/// use std::pin::Pin;
/// use std::task::{Context, Poll, ready};
///
/// struct CoopCounter {
///     id: rio::task::Id,
///     current: usize,
///     max: usize,
/// }
///
/// impl CoopCounter {
///     const fn new(id: rio::task::Id, max: usize) -> Self {
///         Self {
///             id,
///             current: 0,
///             max,
///         }
///     }
///
///     const fn is_complete(&self) -> bool {
///         self.current >= self.max
///     }
/// }
///
/// impl Future for CoopCounter {
///     type Output = ();
///
///     fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
///         while !self.is_complete() {
///             // Ensure there is budget remaining to continue.
///             let coop = ready!(rio::task::coop::poll_proceed());
///
///             println!("task #{}: {}", self.id, self.current);
///             self.current += 1;
///
///             // Progress was made; commit the budget used.
///             coop.made_progress();
///         }
///
///         // Counter has finished; future is complete.
///         Poll::Ready(())
///     }
/// }
/// ```
#[inline]
pub fn poll_proceed() -> Poll<BudgetGuard> {
    context::with_budget(|b| {
        let mut budget = b.get();

        if budget.consume_unit() {
            let guard = BudgetGuard(b.clone());

            b.set(budget);

            Poll::Ready(guard)
        } else {
            context::with_handle(|handle| handle.defer_task(task::id()));
            Poll::Pending
        }
    })
}

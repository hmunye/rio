use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::{any, fmt};

use crate::task::coop;

/// Future returned by [`make_cooperative`].
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Cooperative<F: Future> {
    fut: F,
}

/// Projection type providing a "view" over a `Cooperative<F>`.
struct CooperativeProj<'p, F: Future> {
    fut: Pin<&'p mut F>,
}

impl<F: Future> Cooperative<F> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> CooperativeProj<'_, F> {
        // SAFETY: `self` is a pinned mutable reference to `Cooperative<F>`,
        // making it safe to pin the `fut` field, since `Pin<T>` guarantees that
        // the memory address of this instance will not change.
        unsafe {
            CooperativeProj {
                fut: Pin::new_unchecked(&mut self.get_unchecked_mut().fut),
            }
        }
    }
}

impl<F: Future> Future for Cooperative<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let coop = ready!(coop::poll_proceed());

        let me = self.project();

        if let Poll::Ready(ret) = me.fut.poll(cx) {
            coop.made_progress();
            Poll::Ready(ret)
        } else {
            Poll::Pending
        }
    }
}

impl<F> Unpin for Cooperative<F> where F: Future + Unpin {}

impl<F: Future> fmt::Debug for Cooperative<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cooperative")
            .field("fut", &any::type_name_of_val(&self.fut))
            .finish()
    }
}

/// Enables cooperative scheduling constraints for the given future.
///
/// Unlike [`Unconstrained`], the wrapped future __may__ be forced to yield
/// control to the runtime. This avoids __starvation__, as the task will yield
/// periodically to allow other ready tasks to make progress.
///
/// # Panics
///
/// Panics if the caller `.await` or polls the returned future outside of a
/// runtime context.
///
/// # Examples
///
/// ```no_run
/// # #[rio::main]
/// # async fn main() {
/// use std::future;
///
/// let fut = async {
///     for _ in 0..1_000_000 {
///         // This will always be ready. If cooperative scheduling was not in
///         // effect (i.e., using `rio::task::coop::make_unconstrained`), the
///         // task would not be forced to yield.
///         future::ready(()).await;
///     }
/// };
///
/// rio::task::coop::make_cooperative(fut).await;
/// # }
/// ```
///
/// [`Unconstrained`]: coop::Unconstrained
#[inline]
pub const fn make_cooperative<F: Future>(fut: F) -> Cooperative<F> {
    Cooperative { fut }
}

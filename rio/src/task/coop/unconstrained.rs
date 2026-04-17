use std::pin::Pin;
use std::task::{Context, Poll};
use std::{any, fmt};

use crate::task::coop;

/// Future returned by [`make_unconstrained`].
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Unconstrained<F: Future> {
    fut: F,
}

/// Projection type providing a "view" over an `Unconstrained<F>`.
struct UnconstrainedProj<'p, F: Future> {
    fut: Pin<&'p mut F>,
}

impl<F: Future> Unconstrained<F> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> UnconstrainedProj<'_, F> {
        // SAFETY: `self` is a pinned mutable reference to `Unconstrained<F>`,
        // making it safe to pin the `fut` field, since `Pin<T>` guarantees that
        // the memory address of this instance will not change.
        unsafe {
            UnconstrainedProj {
                fut: Pin::new_unchecked(&mut self.get_unchecked_mut().fut),
            }
        }
    }
}

impl<F: Future> Future for Unconstrained<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        coop::with_unconstrained(|| me.fut.poll(cx))
    }
}

impl<F> Unpin for Unconstrained<F> where F: Future + Unpin {}

impl<F: Future> fmt::Debug for Unconstrained<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Unconstrained")
            .field("fut", &any::type_name_of_val(&self.fut))
            .finish()
    }
}

/// Disables cooperative scheduling constraints for the given future.
///
/// Unlike [`Cooperative`], the wrapped future will __not__ be forced to yield
/// control to the runtime. Failure to yield manually within an unconstrained
/// context can lead to __starvation__ of other tasks.
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
///         // This will always be ready. If cooperative scheduling was in
///         // effect, the task would be forced to yield periodically.
///         future::ready(()).await;
///     }
/// };
///
/// rio::task::coop::make_unconstrained(fut).await;
/// # }
/// ```
///
/// [`Cooperative`]: coop::Cooperative
#[inline]
pub const fn make_unconstrained<F: Future>(fut: F) -> Unconstrained<F> {
    Unconstrained { fut }
}

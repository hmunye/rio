use std::future::Future;

use crate::rt::Runtime;

/// Spawns a new asynchronous task running in the background, enabling it to
/// execute concurrently with other tasks.
///
/// Returning the output of `future` is currently not supported, so it will be
/// polled solely for its side effects.
pub fn spawn<F: Future<Output = ()> + 'static>(future: F) {
    Runtime::current().spawn_inner(future);
}

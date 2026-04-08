cfg_test! {
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    use crate::rt::context;
    use crate::task;

    /// Returns an `Instant` corresponding to "now", accounting for pauses.
    pub fn now() -> Instant {
        context::with_handle(|handle| handle.clock().now())
    }

    /// Pauses logical time.
    ///
    /// # Panics
    ///
    /// Panics if the clock is already paused or if the current thread is not
    /// within a runtime context.
    #[allow(unused)]
    pub fn pause() {
        context::with_handle(|handle| handle.clock().pause());
    }

    /// Advances logical time by `duration`.
    ///
    /// Yields once before advancing so pending tasks can observe the current
    /// time, then yields again after advancing the clock to allow the scheduler
    /// to process given the updated clock.
    ///
    /// # Panics
    ///
    /// Panics if the clock is not paused, if the current thread is not within
    /// a runtime context, or `duration` is too large (e.g, `Duration::MAX`).
    pub async fn advance(duration: Duration) {
        // Pre-yield to ensure tasks spawning `Sleep` futures see the current
        // `clock::now`.
        task::yield_now().await;

        context::with_handle(|handle| handle.clock().advance(duration));

        // Post-yield to ensure the time driver can observe the advanced clock
        // in `clock::now`.
        task::yield_now().await;
    }

    /// Resumes logical time.
    ///
    /// # Panics
    ///
    /// Panics if the clock is not paused or if the current thread is not within
    /// a runtime context.
    pub fn resume() {
        context::with_handle(|handle| {
            let clock = handle.clock();
            assert!(clock.is_paused(), "clock should be paused when resuming");

            clock.resumed_at.replace(Some(Instant::now()));
        });
    }

    /// Source for time abstraction.
    #[derive(Debug)]
    pub struct Clock {
        epoch: Cell<Instant>,
        // Instant at which the clock was last resumed.
        resumed_at: Cell<Option<Instant>>,
    }

    impl Clock {
        #[must_use]
        pub fn new() -> Clock {
            let now = Instant::now();

            let clock = Clock {
                epoch: Cell::new(now),
                resumed_at: Cell::new(Some(now))
            };

            clock.pause();

            clock
        }

        pub fn now(&self) -> Instant {
            let mut now = self.epoch.get();

            if let Some(resumed) = self.resumed_at.get() {
                now += resumed.elapsed();
            }

            now
        }

        pub fn pause(&self) {
            let elapsed = match self.resumed_at.take() {
                Some(v) => v.elapsed(),
                None => panic!("clock is already paused")
            };

            self.epoch.set(self.epoch.get() + elapsed);
        }

        pub fn advance(&self, duration: Duration) {
            assert!(self.is_paused(), "clock should be paused when advancing");
            self.epoch.set(self.epoch.get() + duration);
        }

        fn is_paused(&self) -> bool {
            self.resumed_at.get().is_none()
        }
    }
}

cfg_not_test! {
    use std::time::Instant;

    /// Source for time abstraction.
    #[derive(Debug)]
    pub struct Clock {}

    /// Returns an `Instant` corresponding to "now".
    pub fn now() -> Instant {
        Instant::now()
    }

    impl Clock {
        #[must_use]
        pub const fn new() -> Clock {
            Clock {}
        }

        pub fn now(&self) -> Instant {
            now()
        }
    }
}

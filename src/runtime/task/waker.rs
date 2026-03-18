use std::marker::PhantomData;
use std::rc::Rc;
use std::task::{RawWaker, RawWakerVTable, Waker};

use crate::runtime::{scheduler, task};

/// Wrapper around [`Waker`] that enforces `!Send + !Sync`.
#[derive(Debug)]
pub struct LocalWaker {
    waker: Waker,
    // `Waker` is `Send + Sync` by default.
    _marker: PhantomData<Rc<()>>,
}

#[derive(Debug)]
struct WakerData {
    task_id: task::Id,
    handle: scheduler::Handle,
}

impl LocalWaker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    #[inline]
    pub fn new(task_id: task::Id, handle: scheduler::Handle) -> Self {
        let waker_data = Rc::new(WakerData { task_id, handle });

        LocalWaker {
            // SAFETY: `LocalWaker` does not implement `Send` or `Sync`, so the
            // `RawWaker` data does not need to be thread-safe, allowing us to
            // store `!Send + !Sync` types.
            waker: unsafe { Waker::from_raw(Self::new_raw_waker(waker_data)) },
            _marker: PhantomData,
        }
    }

    #[inline]
    fn new_raw_waker(data: Rc<WakerData>) -> RawWaker {
        RawWaker::new(Rc::into_raw(data).cast::<()>(), &Self::VTABLE)
    }
}

impl std::ops::Deref for LocalWaker {
    type Target = Waker;

    fn deref(&self) -> &Self::Target {
        &self.waker
    }
}

/// Returns a new `RawWaker`, incrementing the reference count of the underlying
/// `Rc<WakerData>`.
unsafe fn clone(ptr: *const ()) -> RawWaker {
    // SAFETY: Raw pointer was created by a call to `Rc::into_raw`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr.cast::<WakerData>()) };

    let cloned = Rc::clone(&data);

    // Ensure `data` isn't dropped to prevent decrementing the reference count.
    std::mem::forget(data);

    LocalWaker::new_raw_waker(cloned)
}

/// Wakes the underlying `Task` and decrements the reference count of
/// `Rc<WakerData>`.
unsafe fn wake(ptr: *const ()) {
    // SAFETY: Raw pointer was created by a call to `Rc::into_raw`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr.cast::<WakerData>()) };

    // Schedule the task for polling.
    data.handle.schedule_task(data.task_id);

    // `data` is dropped here, which decrements the reference count.
}

/// Wakes the underlying `Task` without decrementing the reference count of
/// `Rc<WakerData>`.
unsafe fn wake_by_ref(ptr: *const ()) {
    // SAFETY: Raw pointer was created by a call to `Rc::into_raw`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr.cast::<WakerData>()) };

    // Schedule the task for polling.
    data.handle.schedule_task(data.task_id);

    // Ensure `data` isn't dropped to prevent decrementing the reference count.
    std::mem::forget(data);
}

/// Decrements the reference count of the underlying `Rc<WakerData>`.
unsafe fn drop(ptr: *const ()) {
    // SAFETY: Raw pointer was created by a call to `Rc::into_raw`.
    //
    // Dropping `Rc<WakerData>` here will decrement its reference count.
    let _: Rc<WakerData> = unsafe { Rc::from_raw(ptr.cast::<WakerData>()) };
}

use std::marker::PhantomData;
use std::rc::{Rc, Weak};
use std::task::{RawWaker, RawWakerVTable, Waker};

use crate::rt::Scheduler;
use crate::task;

/// `LocalWaker` is analogous to a [`Waker`], but does not implement [`Send`] or
/// [`Sync`].
///
// FIXME: Use `std::task::LocalWaker` when it is stable.
//
// <https://github.com/rust-lang/rust/issues/118959>
#[derive(Debug)]
pub struct LocalWaker {
    waker: Waker,
    _marker: PhantomData<*mut ()>,
}

#[derive(Debug)]
struct WakerData {
    task_id: task::Id,
    handle: Weak<Scheduler>,
}

impl LocalWaker {
    const WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    pub fn new(task_id: task::Id, handle: Weak<Scheduler>) -> Self {
        let waker_data = WakerData { task_id, handle };

        LocalWaker {
            // SAFETY: All `Wakers` for tasks are constructed from `LocalWaker`,
            // which is `!Send` + `!Sync`, so it's safe for the underlying
            // `RawWaker` data and vtable functions to be !Send + !Sync. After
            // construction, the `Waker` is only accessed from the current
            // thread.
            waker: unsafe { Waker::from_raw(Self::new_raw_waker(Rc::new(waker_data))) },
            _marker: PhantomData,
        }
    }

    fn new_raw_waker(data: Rc<WakerData>) -> RawWaker {
        RawWaker::new(Rc::into_raw(data).cast::<()>(), &Self::WAKER_VTABLE)
    }
}

impl std::ops::Deref for LocalWaker {
    type Target = Waker;

    fn deref(&self) -> &Self::Target {
        &self.waker
    }
}

#[inline]
unsafe fn clone(ptr: *const ()) -> RawWaker {
    // SAFETY: `ptr` was created from a call to `Rc::into_raw`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr.cast::<WakerData>()) };

    let cloned = Rc::clone(&data);

    // Prevent decrementing the reference count.
    std::mem::forget(data);

    LocalWaker::new_raw_waker(cloned)
}

#[inline]
unsafe fn wake(ptr: *const ()) {
    // SAFETY: `ptr` was created from a call to `Rc::into_raw`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr.cast::<WakerData>()) };

    if let Some(handle) = data.handle.upgrade() {
        handle.schedule_task(data.task_id);
    }

    // `data` is dropped here, decrementing the reference count.
}

#[inline]
unsafe fn wake_by_ref(ptr: *const ()) {
    // SAFETY: `ptr` was created from a call to `Rc::into_raw`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr.cast::<WakerData>()) };

    if let Some(handle) = data.handle.upgrade() {
        handle.schedule_task(data.task_id);
    }

    // Prevent decrementing the reference count.
    std::mem::forget(data);
}

#[inline]
unsafe fn drop(ptr: *const ()) {
    // SAFETY: `ptr` was created from a call to `Rc::into_raw`.
    let _data: Rc<WakerData> = unsafe { Rc::from_raw(ptr.cast::<WakerData>()) };

    // `_data` is dropped here, decrementing the reference count.
}

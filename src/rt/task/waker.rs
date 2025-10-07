use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::rc::Rc;
use std::task::{RawWaker, RawWakerVTable, Waker};

use crate::rt::scheduler::Scheduler;
use crate::rt::task::TaskHandle;

/// Wrapper around [`Waker`] that enforces `!Send` and `!Sync`.
#[derive(Debug)]
pub(crate) struct TaskWaker {
    waker: Waker,
    /// `Waker` is `Send` and `Sync` by default. This marker ensures that
    /// `TaskWaker` is `!Send` and `!Sync`, restricting to single-threaded.
    _marker: PhantomData<Rc<()>>,
}

#[derive(Debug)]
struct WakerData {
    task: TaskHandle,
    scheduler: Rc<Scheduler>,
}

impl TaskWaker {
    /// Creates a new `TaskWaker` using the provided [`TaskHandle`] and
    /// [`Scheduler`].
    pub(crate) fn new(task: TaskHandle, scheduler: Rc<Scheduler>) -> Self {
        let waker_data = Rc::new(WakerData { task, scheduler });

        TaskWaker {
            // SAFETY: `TaskWaker` wrapper guarantees it is only usable in a
            // single-threaded context. The vtable functions are only ever
            // called with a valid pointer to the associated underlying `Task`.
            waker: unsafe { Waker::from_raw(Self::raw_waker(waker_data)) },
            _marker: PhantomData,
        }
    }

    fn raw_waker(data: Rc<WakerData>) -> RawWaker {
        // Does not decrement the reference-count of `WakerData`.
        let ptr = Rc::into_raw(data) as *const ();

        RawWaker::new(ptr, &WAKER_VTABLE)
    }
}

impl Deref for TaskWaker {
    type Target = Waker;

    fn deref(&self) -> &Self::Target {
        &self.waker
    }
}

const WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

/// Returns a `RawWaker`, incrementing the reference-count of the underlying
/// `Rc<WakerData>`.
unsafe fn clone(ptr: *const ()) -> RawWaker {
    // SAFETY: Raw pointer was initially created from a valid `Rc<WakerData>`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr as *const WakerData) };
    let cloned = Rc::clone(&data);

    println!("clone: cloning task waker");

    // Prevent `data` from being dropped, which would incorrectly decrement the
    // reference-count.
    mem::forget(data);

    TaskWaker::raw_waker(cloned)
}

/// Wakes the underlying `Task`, consuming the `Rc<WakerData>`.
unsafe fn wake(ptr: *const ()) {
    // SAFETY: Raw pointer was initially created from a valid `Rc<WakerData>`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr as *const WakerData) };

    // Schedule the underlying task for polling
    if !data.task.borrow().scheduled.get() {
        let id = data.task.borrow().id;
        data.scheduler.schedule_task(id);

        println!("wake: waking task {:?}", id);

        // Mark task as scheduled.
        data.task.borrow().scheduled.set(true)
    }

    // `data` is dropped here, as waking by value should consume the `Waker`.
}

/// Wakes the underlying `Task` without consuming the `Rc<WakerData>`.
unsafe fn wake_by_ref(ptr: *const ()) {
    // SAFETY: Raw pointer was initially created from a valid `Rc<WakerData>`.
    let data: Rc<WakerData> = unsafe { Rc::from_raw(ptr as *const WakerData) };

    // Schedule the underlying task for polling
    if !data.task.borrow().scheduled.get() {
        let id = data.task.borrow().id;
        data.scheduler.schedule_task(id);

        println!("wake_by_ref: waking task {:?}", id);

        // Mark task as scheduled.
        data.task.borrow().scheduled.set(true)
    }

    // Waking by reference should not consume the `Waker`.
    mem::forget(data);
}

/// Drops the `Rc` corresponding to the underlying `WakerData`.
unsafe fn drop(ptr: *const ()) {
    // SAFETY: Raw pointer was initially created from a valid `Rc<WakerData>`.
    let _: Rc<WakerData> = unsafe { Rc::from_raw(ptr as *const WakerData) };
}

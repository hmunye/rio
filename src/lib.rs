//! Minimal Asynchronous Runtime for Rust (`rio`).
//!
//! Modern operating systems provide `multitasking`, the capability to
//! interleave the execution of multiple tasks concurrently. The two primary
//! techniques for scheduling tasks are `preemptive` and `cooperative`.
//!
//! **Preemptive multitasking** is the method in which the OS kernel dictates
//! when tasks (`threads`) are scheduled, how long they run, and when they are
//! preempted. Since threads can be interrupted at arbitrary points during
//! execution, the kernel must save the thread's execution state, including the
//! CPU register values, in a process known as a `context switch`. The thread's
//! stack, which is allocated separately within the process's virtual address
//! space, does not need to be saved. This design minimizes the context switch
//! overhead, as the kernel only needs to save and restore CPU registers. The
//! primary advantage of preemptive multitasking is that it allows the kernel to
//! exert fine-grained control over thread execution, ensuring fair scheduling
//! and progress for each thread without reliance on other threads. However,
//! this comes at the cost of increased memory usage per thread, as each
//! requires its own independent stack.
//!
//! **Cooperative multitasking**, in contrast, delegates the responsibility of
//! yielding CPU time to tasks themselves. When combined with asynchronous
//! programming, cooperative multitasking allows tasks to continue executing
//! until they can no longer make progress. As the tasks are responsible for
//! yielding control, they only need to preserve a minimal set of state, such as
//! a subset of local variables, to resume execution later. This reduces the
//! memory overhead per task, as it avoids the need for each task to maintain
//! its own stack. Rust’s `async/await` model leverages this by storing live
//! local variables across suspension points (`await` points) in a compiler
//! generated state machine, allowing multiple tasks to share the same stack.
//! However, the downside of cooperative multitasking is that a misbehaving task
//! can potentially never yield, preventing other tasks from making progress,
//! which can lead to deadlocks if not properly managed.
//!
//! Unlike preemptive multitasking, where the OS kernel schedules tasks and
//! manages context switching, cooperative multitasking requires an explicit
//! `runtime` to manage task scheduling and polling. The runtime is responsible
//! for ensuring tasks are executed in a fair and efficient manner, scheduling
//! them to make progress in accordance with their state transitions and
//! eventual yields.

#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(rust_2018_idioms)]
#![warn(missing_debug_implementations)]
#![allow(clippy::use_self)]
#![allow(clippy::redundant_else)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::struct_excessive_bools)]

pub mod runtime;

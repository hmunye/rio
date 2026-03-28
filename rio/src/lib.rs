//! Minimal Asynchronous Runtime for Rust.
//!
//! `rio` is a lightweight, single-threaded, runtime for building asynchronous
//! applications.
//!
//! ### Multitasking
//!
//! Two primary techniques for interleaving the execution of multiple "tasks"
//! concurrently are **preemptive** and **cooperative**.
//!
//! **Preemptive** multitasking is the method in which the operating system (OS)
//! kernel dictates when tasks (threads) are scheduled, how long they run, and
//! when they are preempted. Since threads can be interrupted at arbitrary
//! points during execution, the kernel must save the thread's execution state,
//! including CPU registers (and maintain a thread control block), in a process
//! known as a **context switch**. The thread's stack, which is allocated
//! separately within the process's virtual address space, does not need to be
//! saved. This design minimizes the context switch overhead, as the kernel only
//! needs to save and restore CPU registers. The primary advantage of this
//! approach is that it allows the kernel to exert fine-grained control over
//! thread execution, ensuring fair scheduling and progress for each thread
//! without the cooperation of other threads. However, this comes at the cost of
//! increased memory usage per thread, as each requires its own independent
//! stack.
//!
//! **Cooperative** multitasking, in contrast, delegates the responsibility of
//! yielding CPU time to tasks themselves. When combined with **asynchronous
//! programming**, cooperative multitasking allows tasks to continue executing
//! until they can no longer make progress. Since the tasks know at which points
//! they will yield control, only a minimal set of state needs to be preserved
//! in order to resume execution at a later point. This reduces the memory
//! footprint per task, as it allows each to share the process's stack. Rust’s
//! `async/await` model leverages this by storing live local variables across
//! suspension points (`await` points) in a compiler-generated state machine.
//! The downside of this approach is that a misbehaving task can starve other
//! tasks or prevent them from making progress all together.
//!
//! Unlike preemptive multitasking, where the OS kernel schedules threads and
//! manages context switches, cooperative multitasking requires an explicit
//! **runtime** to manage task scheduling and polling. It is responsible for
//! ensuring tasks are executed in an efficient manner, scheduling them to make
//! progress fairly.

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
#![allow(clippy::option_if_let_else)]

#[cfg(feature = "rio-macros")]
#[doc(inline)]
pub use rio_macros::main;

pub mod rt;
pub mod task;

pub use task::spawn;

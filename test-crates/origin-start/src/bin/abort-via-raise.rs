//! Test terminating the current process with `Signal::Abort`.

#![no_std]
#![no_main]
#![allow(unused_imports)]

extern crate alloc;

use origin::{program, signal};

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[unsafe(no_mangle)]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    // Terminate the current process using `Signal::ABORT`.
    rustix::process::kill_process(rustix::process::getpid(), signal::Signal::ABORT).ok();
    // We shouldn't get here.
    12
}

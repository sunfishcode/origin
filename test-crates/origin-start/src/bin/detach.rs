//! Test simple `detach` cases.

#![no_std]
#![no_main]

extern crate alloc;

use origin::{program, thread};

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[unsafe(no_mangle)]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 { unsafe {
    let long_thread = thread::create(
        |_args| None,
        &[],
        thread::default_stack_size(),
        thread::default_guard_size(),
    )
    .unwrap();

    let short_thread = thread::create(
        |_args| None,
        &[],
        thread::default_stack_size(),
        thread::default_guard_size(),
    )
    .unwrap();
    thread::detach(short_thread);

    let _thread = thread::create(
        |_args| {
            thread::detach(thread::current());
            None
        },
        &[],
        thread::default_stack_size(),
        thread::default_guard_size(),
    )
    .unwrap();

    thread::detach(long_thread);

    program::exit(202);
}}

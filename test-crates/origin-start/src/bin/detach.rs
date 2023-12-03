//! Test simple `detach` cases.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]

extern crate alloc;
extern crate compiler_builtins;

use atomic_dbg::dbg;
use origin::{program, thread};

#[panic_handler]
fn panic(panic: &core::panic::PanicInfo<'_>) -> ! {
    dbg!(panic);
    core::intrinsics::abort();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
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
}

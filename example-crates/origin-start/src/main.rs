//! A simple example using `no_std` and `no_main` and origin's start.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]
#![feature(naked_functions)]

extern crate alloc;
extern crate compiler_builtins;

use alloc::boxed::Box;
use atomic_dbg::{dbg, eprintln};
use origin::program::*;
use origin::thread::*;

#[panic_handler]
fn panic(panic: &core::panic::PanicInfo<'_>) -> ! {
    dbg!(panic);
    core::intrinsics::abort();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[origin::main]
fn main() -> i32 {
    eprintln!("Hello from main thread");

    at_exit(Box::new(|| eprintln!("Hello from an at_exit handler")));
    at_thread_exit(Box::new(|| {
        eprintln!("Hello from a main-thread at_thread_exit handler")
    }));

    let thread = create_thread(
        Box::new(|| {
            eprintln!("Hello from child thread");
            at_thread_exit(Box::new(|| {
                eprintln!("Hello from child thread's at_thread_exit handler")
            }));
            None
        }),
        2 * 1024 * 1024,
        default_guard_size(),
    )
    .unwrap();

    unsafe {
        join_thread(thread);
    }

    eprintln!("Goodbye from main");
    exit(0);
}

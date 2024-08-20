//! A simple example using `no_std` and `no_main` and origin's start.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]

extern crate alloc;
extern crate compiler_builtins;

use alloc::boxed::Box;
use atomic_dbg::{dbg, eprintln};
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
    eprintln!("Hello from main thread");

    program::at_exit(Box::new(|| {
        eprintln!("Hello from a `program::at_exit` handler")
    }));
    thread::at_exit(Box::new(|| {
        eprintln!("Hello from a main-thread `thread::at_exit` handler")
    }));

    let thread = thread::create(
        |_args| {
            eprintln!("Hello from child thread");
            thread::at_exit(Box::new(|| {
                eprintln!("Hello from child thread's `thread::at_exit` handler")
            }));
            None
        },
        &[],
        thread::default_stack_size(),
        thread::default_guard_size(),
    )
    .unwrap();

    thread::join(thread);

    eprintln!("Goodbye from main");
    program::exit(0);
}

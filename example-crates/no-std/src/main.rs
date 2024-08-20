//! A simple example using `no_std`.

#![no_std]
#![feature(lang_items)]
#![allow(internal_features)]
#![feature(start)]
#![feature(core_intrinsics)]

extern crate alloc;

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

#[start]
fn start(_argc: isize, _argv: *const *const u8) -> isize {
    eprintln!("Hello from main thread");

    program::at_exit(Box::new(|| {
        eprintln!("Hello from a `program::at_exit` handler")
    }));
    thread::at_exit(Box::new(|| {
        eprintln!("Hello from a main-thread `thread::at_exit` handler")
    }));

    let thread = unsafe {
        thread::create(
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
        .unwrap()
    };

    unsafe {
        thread::join(thread);
    }

    eprintln!("Goodbye from main");
    program::exit(0);
}

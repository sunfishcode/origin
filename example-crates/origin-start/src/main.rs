//! A simple example using `no_std` and `no_main` and origin's start.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use atomic_dbg::eprintln;
use origin::{program, thread};

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[unsafe(no_mangle)]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    unsafe {
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
}

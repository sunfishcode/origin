//! A simple example using `no_std` and `no_main` and an external start.

#![no_std]
#![no_main]

extern crate alloc;
extern crate libc;

use alloc::boxed::Box;
use atomic_dbg::eprintln;
use core::sync::atomic::{AtomicBool, Ordering};
use origin::{program, thread};

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

/// Ideally we'd use a mechanism like the `no_entry` feature proposed [here],
/// however the best we can do for now is to let libc start up the program
/// and start running the constructors but to immediately take over.
///
/// [here]: https://github.com/rust-lang/rfcs/pull/2735
#[link_section = ".init_array.00000"]
#[used]
static EARLY_INIT_ARRAY: unsafe extern "C" fn(i32, *mut *mut u8) = {
    unsafe extern "C" fn function(_argc: i32, argv: *mut *mut u8) {
        // Libc was calling constructors (we're one of them), but origin will
        // be doing that now, so just exit when we're called a second time.
        static FIRST: AtomicBool = AtomicBool::new(false);
        if FIRST
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        // Compute the initial stack address provided by the kernel.
        let mem = argv.sub(1);

        origin::program::start(mem as _);
    }
    function
};

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

// Libc calls `main` so we need to provide a definition to satisfy the
// linker, however origin gains control before libc can call this `main`.
#[no_mangle]
unsafe fn main(_argc: i32, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    eprintln!("Main was not supposed to be called!");
    program::trap();
}

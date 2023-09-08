//! A simple example using `no_std` and `no_main` and an external start.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]

extern crate alloc;
extern crate compiler_builtins;
extern crate libc;

use alloc::boxed::Box;
use atomic_dbg::{dbg, eprintln};
use core::sync::atomic::{AtomicBool, Ordering};
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

        origin::start(mem as _);
    }
    function
};

#[no_mangle]
unsafe fn origin_main(_argc: i32, _argv: *const *const u8, _envp: *const *const u8) -> i32 {
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

    join_thread(thread);

    eprintln!("Goodbye from main");
    exit(0);
}

// Libc calls `main` so we need to provide a definition to satisfy the
// linker, however origin gains control before libc can call this `main`.
#[no_mangle]
unsafe fn main(_argc: i32, _argv: *const *const u8, _envp: *const *const u8) -> i32 {
    core::intrinsics::abort();
}

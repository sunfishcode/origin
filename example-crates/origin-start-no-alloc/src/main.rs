//! A simple example using `no_std` and `no_main` and origin's start.

#![no_std]
#![no_main]

use atomic_dbg::eprintln;
use origin::program;

#[unsafe(no_mangle)]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    eprintln!("Hello!");

    // Unlike origin-start, this example can't create threads because origin's
    // thread support requires an allocator.

    program::exit(0);
}

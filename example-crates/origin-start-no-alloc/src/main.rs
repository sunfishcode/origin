//! A simple example using `no_std` and `no_main` and origin's start.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]

extern crate compiler_builtins;

use atomic_dbg::{dbg, eprintln};
use origin::program::*;

#[panic_handler]
fn panic(panic: &core::panic::PanicInfo<'_>) -> ! {
    dbg!(panic);
    core::intrinsics::abort();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[no_mangle]
fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    eprintln!("Hello!");

    // Unlike origin-start, this example can't create threads because origin's
    // thread support requires an allocator.

    exit(0);
}

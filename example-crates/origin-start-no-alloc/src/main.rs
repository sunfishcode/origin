//! A simple example using `no_std` and `no_main` and origin's start.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]
#![feature(naked_functions)]

extern crate compiler_builtins;

use atomic_dbg::{dbg, eprintln};

#[panic_handler]
fn panic(panic: &core::panic::PanicInfo<'_>) -> ! {
    dbg!(panic);
    core::intrinsics::abort();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[origin::main]
fn start() -> i32 {
    eprintln!("Hello!");

    // Unlike origin-start, this example can't create threads because origin's
    // thread support requires an allocator.

    0
}

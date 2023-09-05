//! Going for minimal size!

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]
#![feature(naked_functions)]

extern crate compiler_builtins;
extern crate origin;

#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    core::intrinsics::abort()
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[origin::start]
fn main() -> i32 {
    42
}

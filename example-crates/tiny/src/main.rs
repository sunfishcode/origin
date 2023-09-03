//! Going for minimal size!

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]

extern crate origin;
extern crate compiler_builtins;

#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    core::intrinsics::abort()
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[no_mangle]
fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    42
}

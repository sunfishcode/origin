//! Going for minimal size!

#![no_std]
#![no_main]

extern crate origin;

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    42
}

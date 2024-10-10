//! Test `program::trap`.

#![no_std]
#![no_main]
#![allow(unused_imports)]

extern crate alloc;

use origin::program;

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    program::trap()
}

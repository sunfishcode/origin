//! Test program dtors that add new dtors.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};
use origin::program;

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

static FLAG: AtomicBool = AtomicBool::new(false);

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    program::at_exit(Box::new(|| {
        assert!(FLAG.load(Ordering::Relaxed));
    }));

    program::at_exit(Box::new(|| {
        program::at_exit(Box::new(|| {
            program::at_exit(Box::new(|| {
                FLAG.store(true, Ordering::Relaxed);
            }))
        }))
    }));

    program::at_exit(Box::new(|| {
        assert!(!FLAG.load(Ordering::Relaxed));
    }));

    0
}

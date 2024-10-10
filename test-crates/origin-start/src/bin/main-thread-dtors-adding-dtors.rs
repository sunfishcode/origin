//! Test main thread dtors that add new dtors.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};
use origin::thread;

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

static FLAG: AtomicBool = AtomicBool::new(false);

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    thread::at_exit(Box::new(|| {
        assert!(FLAG.load(Ordering::Relaxed));
    }));

    thread::at_exit(Box::new(|| {
        thread::at_exit(Box::new(|| {
            thread::at_exit(Box::new(|| {
                FLAG.store(true, Ordering::Relaxed);
            }));
        }));
    }));

    thread::at_exit(Box::new(|| {
        assert!(!FLAG.load(Ordering::Relaxed));
    }));

    0
}

//! Like `origin-start`, but add a `#[thread_local]` variable and asserts
//! to ensure that it works.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]
#![feature(thread_local)]
#![feature(strict_provenance)]
#![feature(const_mut_refs)]

extern crate alloc;
extern crate compiler_builtins;

use alloc::boxed::Box;
use atomic_dbg::dbg;
use core::arch::asm;
use core::ptr::{addr_of_mut, invalid_mut};
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

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    // Assert that the main thread initialized its TLS properly.
    check_eq(TEST_DATA.0);

    // Mutate one of the TLS fields.
    THREAD_LOCAL[1] = invalid_mut(77);

    // Assert that the mutation happened properly.
    check_eq([TEST_DATA.0[0], invalid_mut(77), TEST_DATA.0[2]]);

    at_exit(Box::new(|| {
        // This is the last thing to run. Assert that we see the value stored
        // by the `at_thread_exit` callback.
        check_eq([TEST_DATA.0[0], invalid_mut(79), TEST_DATA.0[2]]);

        // Mutate one of the TLS fields.
        THREAD_LOCAL[1] = invalid_mut(80);
    }));
    at_thread_exit(Box::new(|| {
        // Assert that we see the value stored at the end of `main`.
        check_eq([TEST_DATA.0[0], invalid_mut(78), TEST_DATA.0[2]]);

        // Mutate one of the TLS fields.
        THREAD_LOCAL[1] = invalid_mut(79);
    }));

    let thread = create_thread(
        |_args| {
            // Assert that the new thread initialized its TLS properly.
            check_eq(TEST_DATA.0);

            // Mutate one of the TLS fields.
            THREAD_LOCAL[1] = invalid_mut(175);

            // Assert that the mutation happened properly.
            check_eq([TEST_DATA.0[0], invalid_mut(175), TEST_DATA.0[2]]);

            at_thread_exit(Box::new(|| {
                // Assert that we still see the value stored in the thread.
                check_eq([TEST_DATA.0[0], invalid_mut(175), TEST_DATA.0[2]]);
            }));

            None
        },
        &[],
        default_stack_size(),
        default_guard_size(),
    )
    .unwrap();

    join_thread(thread);

    // Assert that the main thread's TLS is still in place.
    check_eq([TEST_DATA.0[0], invalid_mut(77), TEST_DATA.0[2]]);

    // Mutate one of the TLS fields.
    THREAD_LOCAL[1] = invalid_mut(78);

    // Assert that the mutation happened properly.
    check_eq([TEST_DATA.0[0], invalid_mut(78), TEST_DATA.0[2]]);

    exit(200);
}

struct SyncTestData([*const u32; 3]);
unsafe impl Sync for SyncTestData {}
static TEST_DATA: SyncTestData = unsafe {
    SyncTestData([
        invalid_mut(0xa0b1a2b3a4b5a6b7),
        addr_of_mut!(SOME_REGULAR_DATA),
        addr_of_mut!(SOME_ZERO_DATA),
    ])
};

#[thread_local]
static mut THREAD_LOCAL: [*const u32; 3] = TEST_DATA.0;

// Some variables to point to.
static mut SOME_REGULAR_DATA: u32 = 909;
static mut SOME_ZERO_DATA: u32 = 0;

fn check_eq(data: [*const u32; 3]) {
    unsafe {
        // Check `THREAD_LOCAL` using a static address.
        asm!("# {}", in(reg) THREAD_LOCAL.as_mut_ptr(), options(nostack, preserves_flags));
        assert_eq!(THREAD_LOCAL, data);

        // Check `THREAD_LOCAL` using a dynamic address.
        let mut thread_local_addr: *mut [*const u32; 3] = &mut THREAD_LOCAL;
        asm!("# {}", inout(reg) thread_local_addr, options(pure, nomem, nostack, preserves_flags));
        assert_eq!(*thread_local_addr, data);
    }
}

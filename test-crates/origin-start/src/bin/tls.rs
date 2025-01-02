//! Like the `origin-start` example, but add a `#[thread_local]` variable and
//! asserts to ensure that it works.

#![no_std]
#![no_main]
#![feature(thread_local)]

extern crate alloc;

use alloc::boxed::Box;
use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr::{addr_of_mut, without_provenance_mut};
use origin::{program, thread};

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    // Assert that the main thread initialized its TLS properly.
    check_eq(TEST_DATA.0);

    // Mutate one of the TLS fields.
    (*THREAD_LOCAL.get())[1] = without_provenance_mut(77);

    // Assert that the mutation happened properly.
    check_eq([TEST_DATA.0[0], without_provenance_mut(77), TEST_DATA.0[2]]);

    program::at_exit(Box::new(|| {
        // This is the last thing to run. Assert that we see the value stored
        // by the `at_thread_exit` callback.
        check_eq([TEST_DATA.0[0], without_provenance_mut(79), TEST_DATA.0[2]]);

        // Mutate one of the TLS fields.
        (*THREAD_LOCAL.get())[1] = without_provenance_mut(80);
    }));
    thread::at_exit(Box::new(|| {
        // Assert that we see the value stored at the end of `main`.
        check_eq([TEST_DATA.0[0], without_provenance_mut(78), TEST_DATA.0[2]]);

        // Mutate one of the TLS fields.
        (*THREAD_LOCAL.get())[1] = without_provenance_mut(79);
    }));

    let thread = thread::create(
        |_args| {
            // Assert that the new thread initialized its TLS properly.
            check_eq(TEST_DATA.0);

            // Mutate one of the TLS fields.
            (*THREAD_LOCAL.get())[1] = without_provenance_mut(175);

            // Assert that the mutation happened properly.
            check_eq([TEST_DATA.0[0], without_provenance_mut(175), TEST_DATA.0[2]]);

            thread::at_exit(Box::new(|| {
                // Assert that we still see the value stored in the thread.
                check_eq([TEST_DATA.0[0], without_provenance_mut(175), TEST_DATA.0[2]]);
            }));

            None
        },
        &[],
        thread::default_stack_size(),
        thread::default_guard_size(),
    )
    .unwrap();

    thread::join(thread);

    // Assert that the main thread's TLS is still in place.
    check_eq([TEST_DATA.0[0], without_provenance_mut(77), TEST_DATA.0[2]]);

    // Mutate one of the TLS fields.
    (*THREAD_LOCAL.get())[1] = without_provenance_mut(78);

    // Assert that the mutation happened properly.
    check_eq([TEST_DATA.0[0], without_provenance_mut(78), TEST_DATA.0[2]]);

    program::exit(200);
}

struct SyncTestData([*const u32; 3]);
unsafe impl Sync for SyncTestData {}
static TEST_DATA: SyncTestData = {
    SyncTestData([
        without_provenance_mut(0xa0b1a2b3a4b5a6b7_u64 as usize),
        addr_of_mut!(SOME_REGULAR_DATA),
        addr_of_mut!(SOME_ZERO_DATA),
    ])
};

#[thread_local]
static THREAD_LOCAL: UnsafeCell<[*const u32; 3]> = UnsafeCell::new(TEST_DATA.0);

// Some variables to point to.
static mut SOME_REGULAR_DATA: u32 = 909;
static mut SOME_ZERO_DATA: u32 = 0;

fn check_eq(data: [*const u32; 3]) {
    unsafe {
        // Check `THREAD_LOCAL` using a static address.
        asm!("# {}", in(reg) (*THREAD_LOCAL.get()).as_mut_ptr(), options(nostack, preserves_flags));
        assert_eq!(*THREAD_LOCAL.get(), data);

        // Check `THREAD_LOCAL` using a dynamic address.
        let mut thread_local_addr: *mut [*const u32; 3] = THREAD_LOCAL.get();
        asm!("# {}", inout(reg) thread_local_addr, options(pure, nomem, nostack, preserves_flags));
        assert_eq!(*thread_local_addr, data);
    }
}

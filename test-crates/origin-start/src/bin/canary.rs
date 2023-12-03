//! Test stack canaries.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![allow(unused_imports)]
#![feature(lang_items)]
#![feature(core_intrinsics)]

extern crate alloc;
extern crate compiler_builtins;

use atomic_dbg::dbg;
use core::arch::asm;
use origin::{program, thread};

#[panic_handler]
fn panic(panic: &core::panic::PanicInfo<'_>) -> ! {
    dbg!(panic);
    core::intrinsics::abort();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

extern "C" {
    static mut __stack_chk_guard: usize;
}

/// Read the canary field from its well-known location in TLS, if there is one,
/// or read `__stack_chk_guard` otherweise.
fn tls_guard() -> usize {
    let ret;

    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!("mov {}, fs:40", out(reg) ret);
    }

    #[cfg(target_arch = "x86")]
    unsafe {
        asm!("mov {}, gs:20", out(reg) ret);
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    unsafe {
        ret = __stack_chk_guard;
    }

    ret
}

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    assert_ne!(__stack_chk_guard, 0);
    assert_eq!(__stack_chk_guard, tls_guard());

    let thread = thread::create(
        |_args| {
            assert_ne!(__stack_chk_guard, 0);
            assert_eq!(__stack_chk_guard, tls_guard());
            None
        },
        &[],
        thread::default_stack_size(),
        thread::default_guard_size(),
    )
    .unwrap();

    thread::join(thread);

    assert_ne!(__stack_chk_guard, 0);
    assert_eq!(__stack_chk_guard, tls_guard());

    program::exit(203);
}

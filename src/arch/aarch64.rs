#[cfg(any(feature = "origin-thread", feature = "origin-signal"))]
use core::arch::asm;
#[cfg(feature = "origin-signal")]
use linux_raw_sys::general::__NR_rt_sigreturn;
#[cfg(feature = "origin-thread")]
use {
    alloc::boxed::Box,
    core::any::Any,
    core::ffi::c_void,
    linux_raw_sys::general::{__NR_clone, __NR_exit, __NR_munmap},
    rustix::thread::RawPid,
};

/// A wrapper around the Linux `clone` system call.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) unsafe fn clone(
    flags: u32,
    child_stack: *mut c_void,
    parent_tid: *mut RawPid,
    newtls: *mut c_void,
    child_tid: *mut RawPid,
    fn_: *mut Box<dyn FnOnce() -> Option<Box<dyn Any>>>,
) -> isize {
    let r0;
    asm!(
        "svc 0",              // do the `clone` system call
        "cbnz x0,0f",         // branch if we're in the parent thread

        // Child thread.
        "mov x0,{fn_}",       // pass `fn_` as the first argument
        "mov x29, xzr",       // zero the frame address
        "mov x30, xzr",       // zero the return address
        "b {entry}",          // call `entry`

        // Parent thread.
        "0:",

        entry = sym super::thread::entry,
        fn_ = in(reg) fn_,
        in("x8") __NR_clone,
        inlateout("x0") flags as isize => r0,
        in("x1") child_stack,
        in("x2") parent_tid,
        in("x3") newtls,
        in("x4") child_tid,
        options(nostack)
    );
    r0
}

/// Write a value to the platform thread-pointer register.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) unsafe fn set_thread_pointer(ptr: *mut c_void) {
    asm!("msr tpidr_el0,{0}", in(reg) ptr);
    debug_assert_eq!(get_thread_pointer(), ptr);
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) fn get_thread_pointer() -> *mut c_void {
    let ptr;
    unsafe {
        asm!("mrs {0},tpidr_el0", out(reg) ptr, options(nostack, preserves_flags, readonly));
    }
    ptr
}

/// TLS data starts at the location pointed to by the thread pointer.
#[cfg(feature = "origin-thread")]
pub(super) const TLS_OFFSET: usize = 0;

/// `munmap` the current thread, then carefully exit the thread without
/// touching the deallocated stack.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) unsafe fn munmap_and_exit_thread(map_addr: *mut c_void, map_len: usize) -> ! {
    asm!(
        "svc 0",
        "mov x0,xzr",
        "mov x8,{__NR_exit}",
        "svc 0",
        "udf #16",
        __NR_exit = const __NR_exit,
        in("x8") __NR_munmap,
        in("x0") map_addr,
        in("x1") map_len,
        options(noreturn, nostack)
    );
}

/// Invoke the `__NR_rt_sigreturn` system call to return control from a signal
/// handler.
#[cfg(feature = "origin-signal")]
#[naked]
pub(super) unsafe extern "C" fn return_from_signal_handler() {
    asm!(
        "mov x8,{__NR_rt_sigreturn}",
        "svc 0",
        "udf #16",
        __NR_rt_sigreturn = const __NR_rt_sigreturn,
        options(noreturn)
    );
}

/// Invoke the appropriate system call to return control from a signal
/// handler that does not use `SA_SIGINFO`. On aarch64, this uses the same
/// sequence as the `SA_SIGINFO` case.
#[cfg(feature = "origin-signal")]
pub(super) use return_from_signal_handler as return_from_signal_handler_noinfo;

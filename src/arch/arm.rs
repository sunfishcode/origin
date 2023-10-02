//! Architecture-specific assembly code.

use core::arch::asm;
#[cfg(feature = "origin-signal")]
use linux_raw_sys::general::{__NR_rt_sigreturn, __NR_sigreturn};
#[cfg(feature = "origin-thread")]
use {
    alloc::boxed::Box,
    core::any::Any,
    core::ffi::c_void,
    linux_raw_sys::general::{__NR_clone, __NR_exit, __NR_munmap},
    rustix::thread::RawPid,
};

/// The program entry point.
///
/// # Safety
///
/// This function must never be called explicitly. It is the first thing
/// executed in the program, and it assumes that memory is laid out according
/// to the operating system convention for starting a new program.
#[cfg(feature = "origin-start")]
#[naked]
#[no_mangle]
pub(super) unsafe extern "C" fn _start() -> ! {
    // Jump to `entry`, passing it the initial stack pointer value as an
    // argument, a null return address, a null frame pointer, and an aligned
    // stack pointer. On many architectures, the incoming frame pointer is
    // already null.
    asm!(
        "mov r0, sp\n", // Pass the incoming `sp` as the arg to `entry`.
        "mov lr, #0",   // Set the return address to zero.
        "b {entry}",    // Jump to `entry`.
        entry = sym super::program::entry,
        options(noreturn),
    )
}

/// A wrapper around the Linux `clone` system call.
///
/// This can't be implemented in `rustix` because the child starts executing at
/// the same point as the parent and we need to use inline asm to have the
/// child jump to our new-thread entrypoint.
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
        "tst r0,r0",          // branch if we're in the parent thread
        "bne 0f",

        // Child thread.
        "mov r0,{fn_}",       // pass `fn_` as the first argument
        "mov fp, #0",         // zero the frame address
        "mov lr, #0",         // zero the return address
        "b {entry}",

        // Parent thread.
        "0:",

        entry = sym super::thread::entry,
        fn_ = in(reg) fn_,
        in("r7") __NR_clone,
        inlateout("r0") flags as usize => r0,
        in("r1") child_stack,
        in("r2") parent_tid,
        in("r3") newtls,
        in("r4") child_tid,
        options(nostack)
    );
    r0
}

/// Write a value to the platform thread-pointer register.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) unsafe fn set_thread_pointer(ptr: *mut c_void) {
    rustix::runtime::arm_set_tls(ptr).expect("arm_set_tls");
    debug_assert_eq!(get_thread_pointer(), ptr);
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) fn get_thread_pointer() -> *mut c_void {
    let ptr;
    unsafe {
        asm!("mrc p15,0,{},c13,c0,3", out(reg) ptr);
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
        "mov r0,#0",
        "mov r7,{__NR_exit}",
        "svc 0",
        "udf #16",
        __NR_exit = const __NR_exit,
        in("r7") __NR_munmap,
        in("r0") map_addr,
        in("r1") map_len,
        options(noreturn, nostack)
    );
}

/// Invoke the `__NR_rt_sigreturn` system call to return control from a signal
/// handler.
///
/// # Safety
///
/// This function must never be called other than by the `sa_restorer`
/// mechanism.
#[cfg(feature = "origin-signal")]
#[naked]
pub(super) unsafe extern "C" fn return_from_signal_handler() {
    asm!(
        "mov r7,{__NR_rt_sigreturn}",
        "swi 0",
        "udf #16",
        __NR_rt_sigreturn = const __NR_rt_sigreturn,
        options(noreturn)
    );
}

/// Invoke the appropriate system call to return control from a signal
/// handler that does not use `SA_SIGINFO`.
///
/// # Safety
///
/// This function must never be called other than by the `sa_restorer`
/// mechanism.
#[cfg(feature = "origin-signal")]
#[naked]
pub(super) unsafe extern "C" fn return_from_signal_handler_noinfo() {
    asm!(
        "mov r7,{__NR_sigreturn}",
        "swi 0",
        "udf #16",
        __NR_sigreturn = const __NR_sigreturn,
        options(noreturn)
    );
}

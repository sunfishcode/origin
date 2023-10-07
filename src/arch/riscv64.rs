//! Architecture-specific assembly code.

#[cfg(any(feature = "origin-thread", feature = "origin-start"))]
use core::arch::asm;
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
        "mv a0, sp",    // Pass the incoming `sp` as the arg to `entry`.
        "mv ra, zero",  // Set the return address to zero.
        "mv fp, zero",  // Set the frame address to zero.
        "tail {entry}", // Jump to `entry`.
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
        "ecall",              // Do the `clone` system call.
        "bnez a0, 0f",        // Branch if we're in the parent thread.

        // Child thread.
        "mv a0, {fn_}",       // Pass `fn_` as the first argument.
        "mv fp, zero",        // Zero the frame address.
        "mv ra, zero",        // Zero the return address.
        "tail {entry}",       // Call `entry`.

        // Parent thread.
        "0:",

        entry = sym super::thread::entry,
        fn_ = in(reg) fn_,
        in("a7") __NR_clone,
        inlateout("a0") flags as usize => r0,
        in("a1") child_stack,
        in("a2") parent_tid,
        in("a3") newtls,
        in("a4") child_tid,
        options(nostack)
    );
    r0
}

/// Write a value to the platform thread-pointer register.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) unsafe fn set_thread_pointer(ptr: *mut c_void) {
    asm!("mv tp, {}", in(reg) ptr);
    debug_assert_eq!(get_thread_pointer(), ptr);
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) fn get_thread_pointer() -> *mut c_void {
    let ptr;
    unsafe {
        asm!("mv {}, tp", out(reg) ptr, options(nostack, preserves_flags, readonly));
    }
    ptr
}

/// TLS data starts 0x800 bytes below the location pointed to by the thread
/// pointer.
#[cfg(feature = "origin-thread")]
pub(super) const TLS_OFFSET: usize = 0x800;

/// `munmap` the current thread, then carefully exit the thread without
/// touching the deallocated stack.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) unsafe fn munmap_and_exit_thread(map_addr: *mut c_void, map_len: usize) -> ! {
    asm!(
        "ecall",
        "mv a0, zero",
        "li a7, {__NR_exit}",
        "ecall",
        "unimp",
        __NR_exit = const __NR_exit,
        in("a7") __NR_munmap,
        in("a0") map_addr,
        in("a1") map_len,
        options(noreturn, nostack)
    );
}

// RISC-V doesn't use `__NR_rt_sigreturn`

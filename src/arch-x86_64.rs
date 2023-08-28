#[cfg(any(feature = "origin-thread", feature = "origin-signal"))]
use core::arch::asm;
#[cfg(feature = "thread")]
use core::ffi::c_void;
#[cfg(feature = "origin-signal")]
use linux_raw_sys::general::__NR_rt_sigreturn;
#[cfg(feature = "origin-thread")]
use {
    alloc::boxed::Box,
    core::any::Any,
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
    child_tid: *mut RawPid,
    newtls: *mut c_void,
    fn_: *mut Box<dyn FnOnce() -> Option<Box<dyn Any>>>,
) -> isize {
    let r0;
    asm!(
        "syscall",
        "test eax,eax",
        "jnz 0f",

        // Child thread.
        "xor ebp,ebp",     // zero the frame address
        "mov rdi,r9",      // `fn_`
        "push rax",        // zero the return address
        "jmp {entry}",

        // Parent thread.
        "0:",

        entry = sym super::thread::entry,
        inlateout("rax") __NR_clone as isize => r0,
        in("rdi") flags,
        in("rsi") child_stack,
        in("rdx") parent_tid,
        in("r10") child_tid,
        in("r8") newtls,
        in("r9") fn_,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    r0
}

/// Write a value to the platform thread-pointer register.
#[cfg(all(
    feature = "origin-thread",
    any(feature = "origin-start", feature = "external-start")
))]
#[inline]
pub(super) unsafe fn set_thread_pointer(ptr: *mut c_void) {
    rustix::runtime::set_fs(ptr);
    debug_assert_eq!(*ptr.cast::<*const c_void>(), ptr);
    debug_assert_eq!(get_thread_pointer(), ptr);
}

/// Read the value of the platform thread-pointer register.
#[cfg(all(
    feature = "origin-thread",
    any(feature = "origin-start", feature = "external-start")
))]
#[inline]
pub(super) fn get_thread_pointer() -> *mut c_void {
    let ptr;
    unsafe {
        asm!("mov {},QWORD PTR fs:0", out(reg) ptr, options(nostack, preserves_flags, readonly));
    }
    ptr
}

/// TLS data ends at the location pointed to by the thread pointer.
#[cfg(feature = "origin-thread")]
pub(super) const TLS_OFFSET: usize = 0;

/// `munmap` the current thread, then carefully exit the thread without
/// touching the deallocated stack.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) unsafe fn munmap_and_exit_thread(map_addr: *mut c_void, map_len: usize) -> ! {
    asm!(
        "syscall",
        "xor edi,edi",
        "mov eax,{__NR_exit}",
        "syscall",
        "ud2",
        __NR_exit = const __NR_exit,
        in("rax") __NR_munmap,
        in("rdi") map_addr,
        in("rsi") map_len,
        options(noreturn, nostack)
    );
}

/// Invoke the `__NR_rt_sigreturn` system call to return control from a signal
/// handler.
#[cfg(feature = "origin-signal")]
#[naked]
pub(super) unsafe extern "C" fn return_from_signal_handler() {
    asm!(
        "mov rax,{__NR_rt_sigreturn}",
        "syscall",
        "ud2",
        __NR_rt_sigreturn = const __NR_rt_sigreturn,
        options(noreturn)
    );
}

/// Invoke the appropriate system call to return control from a signal
/// handler that does not use `SA_SIGINFO`.
#[cfg(feature = "origin-signal")]
pub(super) use return_from_signal_handler as return_from_signal_handler_noinfo;

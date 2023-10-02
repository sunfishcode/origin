//! Architecture-specific assembly code.

use core::arch::asm;
#[cfg(feature = "origin-signal")]
use linux_raw_sys::general::{__NR_rt_sigreturn, __NR_sigreturn};
#[cfg(feature = "origin-thread")]
use {
    alloc::boxed::Box,
    core::any::Any,
    core::ffi::c_void,
    core::ptr::invalid,
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
        "mov eax, esp", // Save the incoming `esp` value.
        "push ebp",     // Pad for stack pointer alignment.
        "push ebp",     // Pad for stack pointer alignment.
        "push ebp",     // Pad for stack pointer alignment.
        "push eax",     // Pass saved the incoming `esp` as the arg to `entry`.
        "push ebp",     // Set the return address to zero.
        "jmp {entry}",  // Jump to `entry`.
        entry = sym super::program::entry,
        options(noreturn),
    )
}

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
    let mut gs: u32 = 0;
    asm!("mov {0:x},gs", inout(reg) gs);
    let entry_number = gs >> 3;

    let mut user_desc = rustix::runtime::UserDesc {
        entry_number,
        base_addr: newtls.addr() as u32,
        limit: 0xfffff,
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        __bindgen_padding_0: [0_u8; 3_usize],
    };
    user_desc.set_seg_32bit(1);
    user_desc.set_contents(0);
    user_desc.set_read_exec_only(0);
    user_desc.set_limit_in_pages(1);
    user_desc.set_seg_not_present(0);
    user_desc.set_useable(1);
    let newtls: *const _ = &user_desc;

    // See the comments for x86's `syscall6` in `rustix`. Inline asm isn't
    // allowed to name ebp or esi as operands, so we have to jump through
    // extra hoops here.
    let r0;
    asm!(
        "push esi",           // save incoming register value
        "push ebp",           // save incoming register value

        // Pass `fn_` to the child in ebp.
        "mov ebp,DWORD PTR [eax+8]",

        "mov esi, [eax + 0]", // `newtls`
        "mov eax, [eax + 4]", // `__NR_clone`

        // Use `int 0x80` instead of vsyscall, following `clone`'s
        // documentation; vsyscall would attempt to return to the parent stack
        // in the child.
        "int 0x80",           // do the `clone` system call
        "test eax,eax",       // branch if we're in the parent
        "jnz 0f",

        // Child thread.
        "push eax",           // pad for alignment
        "push eax",           // pad for alignment
        "push eax",           // pad for alignment
        "push ebp",           // pass `fn` as the first argument
        "xor ebp,ebp",        // zero the frame address
        "push eax",           // zero the return address
        "jmp {entry}",

        // Parent thread.
        "0:",
        "pop ebp",            // restore incoming register value
        "pop esi",            // restore incoming register value

        entry = sym super::thread::entry,
        inout("eax") &[newtls as *mut c_void, invalid(__NR_clone as usize), fn_ as *mut c_void] => r0,
        in("ebx") flags,
        in("ecx") child_stack,
        in("edx") parent_tid,
        in("edi") child_tid,
    );
    r0
}

/// Write a value to the platform thread-pointer register.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) unsafe fn set_thread_pointer(ptr: *mut c_void) {
    let mut user_desc = rustix::runtime::UserDesc {
        entry_number: !0u32,
        base_addr: ptr.addr() as u32,
        limit: 0xfffff,
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        __bindgen_padding_0: [0_u8; 3_usize],
    };
    user_desc.set_seg_32bit(1);
    user_desc.set_contents(0);
    user_desc.set_read_exec_only(0);
    user_desc.set_limit_in_pages(1);
    user_desc.set_seg_not_present(0);
    user_desc.set_useable(1);
    rustix::runtime::set_thread_area(&mut user_desc).expect("set_thread_area");
    asm!("mov gs,{0:x}", in(reg) ((user_desc.entry_number << 3) | 3) as u16);
    debug_assert_eq!(*ptr.cast::<*const c_void>(), ptr);
    debug_assert_eq!(get_thread_pointer(), ptr);
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "origin-thread")]
#[inline]
pub(super) fn get_thread_pointer() -> *mut c_void {
    let ptr;
    unsafe {
        asm!("mov {},DWORD PTR gs:0", out(reg) ptr, options(nostack, preserves_flags, readonly));
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
        // Use `int 0x80` instead of vsyscall, since vsyscall would attempt to
        // touch the stack after we `munmap` it.
        "int 0x80",
        "xor ebx,ebx",
        "mov eax,{__NR_exit}",
        "int 0x80",
        "ud2",
        __NR_exit = const __NR_exit,
        in("eax") __NR_munmap,
        in("ebx") map_addr,
        in("ecx") map_len,
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
        "mov eax,{__NR_rt_sigreturn}",
        "int 0x80",
        "ud2",
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
        "pop eax",
        "mov eax, {__NR_sigreturn}",
        "int 0x80",
        "ud2",
        __NR_sigreturn = const __NR_sigreturn,
        options(noreturn)
    );
}

//! Architecture-specific assembly code.

#[cfg(any(
    feature = "take-charge",
    all(not(feature = "unwinding"), feature = "panic-handler-trap")
))]
use core::arch::asm;
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
use linux_raw_sys::elf::Elf_Ehdr;
#[cfg(feature = "take-charge")]
#[cfg(feature = "signal")]
use linux_raw_sys::general::__NR_rt_sigreturn;
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
use linux_raw_sys::general::{__NR_mprotect, PROT_READ};
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
use {
    core::ffi::c_void,
    linux_raw_sys::general::{__NR_clone, __NR_exit, __NR_munmap},
    rustix::thread::RawPid,
};

#[cfg(feature = "origin-start")]
naked_fn!(
    "
    The program entry point.

    # Safety

    This function must never be called explicitly. It is the first thing
    executed in the program, and it assumes that memory is laid out according
    to the operating system convention for starting a new program.
    ";
    pub(super) fn _start() -> !;

    // Jump to `entry`, passing it the initial stack pointer value as an
    // argument, a null return address, a null frame pointer, and an aligned
    // stack pointer. On many architectures, the incoming frame pointer is
    // already null.
    "mov rdi, rsp", // Pass the incoming `rsp` as the arg to `entry`.
    "push rbp",     // Set the return address to zero.
    "jmp {entry}";  // Jump to `entry`.
    entry = sym super::program::entry
);

/// Execute a trap instruction.
///
/// This is roughly equivalent to `core::intrinsics::abort()`.
#[cfg(any(
    feature = "take-charge",
    all(not(feature = "unwinding"), feature = "panic-handler-trap")
))]
pub(super) fn trap() -> ! {
    unsafe {
        asm!("ud2", options(noreturn, nostack));
    }
}

/// Compute the dynamic address of `_DYNAMIC`.
#[cfg(any(
    all(feature = "thread", feature = "take-charge"),
    all(feature = "experimental-relocate", feature = "origin-start")
))]
#[allow(dead_code)]
pub(super) fn dynamic_table_addr() -> *const linux_raw_sys::elf::Elf_Dyn {
    let addr;
    unsafe {
        asm!(
            ".weak _DYNAMIC",
            ".hidden _DYNAMIC",
            "lea {}, [rip + _DYNAMIC]",
            out(reg) addr
        );
    }
    addr
}

/// Compute the dynamic address of `__ehdr_start`.
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
pub(super) fn ehdr_addr() -> *const Elf_Ehdr {
    let addr: *const Elf_Ehdr;
    unsafe {
        asm!(
            "lea {}, [rip + __ehdr_start]",
            out(reg) addr
        );
    }
    addr
}

/// Perform a single load operation, outside the Rust memory model.
///
/// This function conceptually casts `ptr` to a `*const *mut c_void` and loads
/// a `*mut c_void` value from it. However, it does this using `asm`, and
/// `usize` types which don't carry provenance, as it's used by `relocate` to
/// perform relocations which cannot be expressed in the Rust memory model.
///
/// # Safety
///
/// This function must only be called during the relocation process, for
/// relocation purposes. And, `ptr` must contain the address of a memory
/// location that can be loaded from.
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
#[inline]
pub(super) unsafe fn relocation_load(ptr: usize) -> usize {
    let r0;

    // This is read-only but we don't use `readonly` because this memory access
    // happens outside the Rust memory model. As far as Rust knows, this is
    // just an arbitrary side-effecting opaque operation.
    asm!(
        "mov {}, [{}]",
        out(reg) r0,
        in(reg) ptr,
        options(nostack, preserves_flags),
    );

    r0
}

/// Perform a single store operation, outside the Rust memory model.
///
/// This function conceptually casts `ptr` to a `*mut *mut c_void` and stores
/// `value` casted to `*mut c_void` through it. However, it does this using
/// `asm`, and `usize` types which don't carry provenance, as it's used by
/// `relocate` to perform relocations which cannot be expressed in the Rust
/// memory model.
///
/// # Safety
///
/// This function must only be called during the relocation process, for
/// relocation purposes. And, `ptr` must contain the address of a memory
/// location that can be stored to.
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
#[inline]
pub(super) unsafe fn relocation_store(ptr: usize, value: usize) {
    asm!(
        "mov [{}], {}",
        in(reg) ptr,
        in(reg) value,
        options(nostack, preserves_flags),
    );
}

/// Mark “relro” memory as readonly.
///
/// “relro” is a relocation feature in which memory can be readonly after
/// relocations are applied.
///
/// This function conceptually casts `ptr` to a `*mut c_void` and does a
/// `rustix::mm::mprotect(ptr, len, MprotectFlags::READ)`. However, it does
/// this using `asm` and `usize` types which don't carry provenance, as it's
/// used by `relocate` to implement the “relro” feature which cannot be
/// expressed in the Rust memory model.
///
/// # Safety
///
/// This function must only be called during the relocation process, for
/// relocation purposes. And, `ptr` must contain the address of a memory
/// location that can be marked readonly.
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
#[inline]
pub(super) unsafe fn relocation_mprotect_readonly(ptr: usize, len: usize) {
    let r0: usize;

    // This is read-only but we don't use `readonly` because the side effects
    // happen outside the Rust memory model. As far as Rust knows, this is
    // just an arbitrary side-effecting opaque operation.
    asm!(
        "syscall",
        inlateout("rax") __NR_mprotect as usize => r0,
        in("rdi") ptr,
        in("rsi") len,
        in("rdx") PROT_READ,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );

    if r0 != 0 {
        // Do not panic here as libstd's panic handler needs TLS, which is not
        // yet initialized at this point.
        trap();
    }
}

/// The required alignment for the stack pointer.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
pub(super) const STACK_ALIGNMENT: usize = 16;

/// A wrapper around the Linux `clone` system call.
///
/// This can't be implemented in `rustix` because the child starts executing at
/// the same point as the parent and we need to use inline asm to have the
/// child jump to our new-thread entrypoint.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) unsafe fn clone(
    flags: u32,
    child_stack: *mut c_void,
    parent_tid: *mut RawPid,
    child_tid: *mut RawPid,
    newtls: *mut c_void,
    fn_: extern "C" fn(),
    num_args: usize,
) -> isize {
    let r0;
    asm!(
        "syscall",            // Do the `clone` system call.
        "test eax, eax",      // Branch if we're in the parent thread.
        "jnz 2f",

        // Child thread.
        "mov rdi, r9",        // Pass `fn_` as the first argument.
        "mov rsi, rsp",       // Pass the args pointer as the second argument.
        "mov rdx, r12",       // Pass `num_args` as the third argument.
        "xor ebp, ebp",       // Zero the frame address.
        "push rax",           // Zero the return address.
        "jmp {entry}",        // Call `entry`.

        // Parent thread.
        "2:",

        entry = sym super::thread::entry,
        inlateout("rax") __NR_clone as usize => r0,
        in("rdi") flags,
        in("rsi") child_stack,
        in("rdx") parent_tid,
        in("r10") child_tid,
        in("r8") newtls,
        in("r9") fn_,
        in("r12") num_args,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    r0
}

/// Write a value to the platform thread-pointer register.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) unsafe fn set_thread_pointer(ptr: *mut c_void) {
    rustix::runtime::set_fs(ptr);
    debug_assert_eq!(*ptr.cast::<*const c_void>(), ptr);
    debug_assert_eq!(thread_pointer(), ptr);
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) fn thread_pointer() -> *mut c_void {
    let ptr;
    // SAFETY: On x86_64, reading the thread register itself is expensive, so
    // the ABI specifies that the thread pointer value is also stored in memory
    // at offset 0 from the thread pointer value, where it can be read with
    // just a load.
    unsafe {
        asm!("mov {}, fs:0", out(reg) ptr, options(nostack, preserves_flags, readonly));
    }
    ptr
}

/// TLS data ends at the location pointed to by the thread pointer.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
pub(super) const TLS_OFFSET: usize = 0;

/// `munmap` the current thread, then carefully exit the thread without
/// touching the deallocated stack.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) unsafe fn munmap_and_exit_thread(map_addr: *mut c_void, map_len: usize) -> ! {
    asm!(
        "syscall",
        "xor edi, edi",
        "mov eax, {__NR_exit}",
        "syscall",
        "ud2",
        __NR_exit = const __NR_exit,
        in("rax") __NR_munmap,
        in("rdi") map_addr,
        in("rsi") map_len,
        options(noreturn, nostack)
    );
}

#[cfg(feature = "take-charge")]
#[cfg(feature = "signal")]
naked_fn!(
    "
    Invoke the `__NR_rt_sigreturn` system call to return control from a signal
    handler.

    # Safety

    This function must never be called other than by the `sa_restorer`
    mechanism.
    ";
    pub(super) fn return_from_signal_handler() -> ();

    "mov rax, {__NR_rt_sigreturn}",
    "syscall",
    "ud2";
    __NR_rt_sigreturn = const __NR_rt_sigreturn
);

/// Invoke the appropriate system call to return control from a signal
/// handler that does not use `SA_SIGINFO`. On x86-64, this uses the same
/// sequence as the `SA_SIGINFO` case.
///
/// # Safety
///
/// This function must never be called other than by the `sa_restorer`
/// mechanism.
#[cfg(feature = "take-charge")]
#[cfg(feature = "signal")]
pub(super) use return_from_signal_handler as return_from_signal_handler_noinfo;

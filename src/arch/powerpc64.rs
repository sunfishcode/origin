//! Architecture-specific assembly code for PowerPC64 (ELF v2 ABI).

#[cfg(any(
    feature = "take-charge",
    all(not(feature = "unwinding"), feature = "panic-handler-trap")
))]
use core::arch::asm;
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
use linux_raw_sys::elf::Elf_Ehdr;
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

/// The program entry point.
///
/// # Safety
///
/// This function must never be called explicitly. It is the first thing
/// executed in the program, and it assumes that memory is laid out according
/// to the operating system convention for starting a new program.
///
/// PowerPC64 ELF v2 ABI: r1 holds sp, r12 holds entry point address,
/// r3-r10 are argument registers.
#[cfg(feature = "origin-start")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub(super) unsafe extern "C" fn _start() -> ! {
    // Jump to `entry`, passing it the initial stack pointer value as an
    // argument, a null return address, a null frame pointer, and an aligned
    // stack pointer.
    //
    // PPC64 ELF v2: r1 = sp, r12 = entry point address.
    // We must set up r2 (TOC pointer) before calling Rust code that uses
    // global data. In ELFv2, r2 should point to .TOC. which is computed
    // relative to the entry point using r12.
    core::arch::naked_asm!(
        // Set up r2 (TOC pointer) - required for accessing global data.
        // r12 contains the address of _start at program entry.
        "addis 2, 12, .TOC. - _start@ha",
        "addi 2, 2, .TOC. - _start@l",
        "mr 3, 1",      // Pass the incoming `sp` (r1) as the arg to `entry` (r3).
        "li 0, 0",      // Clear r0.
        "mtlr 0",       // Set the link register (return address) to zero.
        "b {entry}",    // Jump to `entry`.
        entry = sym super::program::entry
    )
}

/// Execute a trap instruction.
///
/// This is roughly equivalent to `core::intrinsics::abort()`.
#[cfg(any(
    feature = "take-charge",
    all(not(feature = "unwinding"), feature = "panic-handler-trap")
))]
pub(super) fn trap() -> ! {
    unsafe {
        // The `trap` instruction is an unconditional trap on PowerPC.
        asm!("trap", options(noreturn, nostack));
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
            // Use PC-relative addressing to get _DYNAMIC address
            "addis {0}, 2, _DYNAMIC@toc@ha",
            "addi {0}, {0}, _DYNAMIC@toc@l",
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
            "addis {0}, 2, __ehdr_start@toc@ha",
            "addi {0}, {0}, __ehdr_start@toc@l",
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
    unsafe {
        let r0;

        // This is read-only but we don't use `readonly` because this memory access
        // happens outside the Rust memory model. As far as Rust knows, this is
        // just an arbitrary side-effecting opaque operation.
        asm!(
            "ld {0}, 0({1})",
            out(reg) r0,
            in(reg) ptr,
            options(nostack, preserves_flags),
        );

        r0
    }
}

/// Perform a raw load operation to memory that Rust may consider out of
/// bounds.
///
/// Data loaded from out-of-bounds bytes will have nondeterministic values.
///
/// # Safety
///
/// `ptr` must be aligned for loading a `usize` and must point to enough
/// readable memory for loading a `usize`.
#[cfg(all(feature = "take-charge", not(feature = "optimize_for_size")))]
#[inline]
pub(super) unsafe fn oob_load(ptr: *const usize) -> usize {
    unsafe {
        let r0;

        asm!(
            "ld {0}, 0({1})",
            out(reg) r0,
            in(reg) ptr,
            options(nostack, preserves_flags, readonly),
        );

        r0
    }
}

/// Perform a single store operation, outside the Rust memory model.
///
/// This function conceptually casts `ptr` to a `*mut *mut c_void` and stores
/// a `*mut c_void` value to it. However, it does this using `asm`, and `usize`
/// types which don't carry provenance, as it's used by `relocate` to perform
/// relocations which cannot be expressed in the Rust memory model.
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
    unsafe {
        asm!(
            "std {0}, 0({1})",
            in(reg) value,
            in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
}

/// Mark "relro" memory as readonly.
///
/// "relro" is a relocation feature in which memory can be readonly after
/// relocations are applied.
///
/// This function conceptually casts `ptr` to a `*mut c_void` and does a
/// `rustix::mm::mprotect(ptr, len, MprotectFlags::READ)`. However, it does
/// this using `asm` and `usize` types which don't carry provenance, as it's
/// used by `relocate` to implement the "relro" feature which cannot be
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
    unsafe {
        let r0: usize;

        // PowerPC64 syscall: r0 = syscall number, r3-r8 = args, return in r3.
        // cr0.SO indicates error.
        asm!(
            "sc",
            "bso 1f",           // Branch if SO (summary overflow) is set (error)
            "li {0}, 0",        // Success: set r0 to 0
            "b 2f",
            "1:",
            "mr {0}, 3",        // Error: copy error code from r3
            "2:",
            out(reg) r0,
            in("0") __NR_mprotect,
            in("3") ptr,
            in("4") len,
            in("5") PROT_READ,
            lateout("4") _,
            lateout("5") _,
            lateout("6") _,
            lateout("7") _,
            lateout("8") _,
            lateout("9") _,
            lateout("10") _,
            lateout("11") _,
            lateout("12") _,
            lateout("cr0") _,
            options(nostack),
        );

        if r0 != 0 {
            // Do not panic here as libstd's panic handler needs TLS, which is not
            // yet initialized at this point.
            trap();
        }
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
    unsafe {
        let r0;
        asm!(
            "sc",                 // Do the `clone` system call.
            "cmpdi 3, 0",         // Compare r3 to 0.
            "bne 2f",             // Branch if we're in the parent thread (r3 != 0).

            // Child thread.
            "mr 3, {fn_}",        // Pass `fn_` as the first argument.
            "mr 4, 1",            // Pass the stack pointer as the second argument.
            "mr 5, {num_args}",   // Pass `num_args` as the third argument.
            "li 0, 0",            // Clear r0.
            "mtlr 0",             // Zero the return address.
            "b {entry}",          // Jump to `entry`.

            // Parent thread.
            "2:",

            entry = sym super::thread::entry,
            fn_ = in(reg) fn_,
            num_args = in(reg) num_args,
            in("0") __NR_clone,
            inlateout("3") flags as usize => r0,
            in("4") child_stack,
            in("5") parent_tid,
            in("6") child_tid,
            in("7") newtls,
            lateout("8") _,
            lateout("9") _,
            lateout("10") _,
            lateout("11") _,
            lateout("12") _,
            lateout("cr0") _,
            options(nostack)
        );
        r0
    }
}

/// Write a value to the platform thread-pointer register.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) unsafe fn set_thread_pointer(ptr: *mut c_void) {
    unsafe {
        // On PowerPC64, r13 is the thread pointer register.
        asm!("mr 13, {}", in(reg) ptr);
        debug_assert_eq!(thread_pointer(), ptr);
    }
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) fn thread_pointer() -> *mut c_void {
    let ptr;
    // SAFETY: This reads the thread register (r13).
    unsafe {
        asm!("mr {}, 13", out(reg) ptr, options(nostack, preserves_flags, readonly));
    }
    ptr
}

/// TLS data starts at a negative offset from the thread pointer on PowerPC64.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
pub(super) const TLS_OFFSET: usize = 0x7000;

/// `munmap` the current thread, then carefully exit the thread without
/// touching the deallocated stack.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) unsafe fn munmap_and_exit_thread(map_addr: *mut c_void, map_len: usize) -> ! {
    unsafe {
        asm!(
            "sc",                   // munmap syscall
            "li 3, 0",              // exit code 0
            "li 0, {__NR_exit}",    // exit syscall number
            "sc",                   // exit syscall
            "trap",                 // should never reach here
            __NR_exit = const __NR_exit,
            in("0") __NR_munmap,
            in("3") map_addr,
            in("4") map_len,
            options(noreturn, nostack)
        );
    }
}

// PowerPC64 doesn't use sa_restorer for signal handling

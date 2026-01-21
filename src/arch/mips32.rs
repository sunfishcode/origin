//! Architecture-specific assembly code for MIPS32 (O32 ABI).

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
    linux_raw_sys::general::{
        __NR_clone, __NR_exit, __NR_munmap, __NR_set_thread_area, CLONE_CHILD_CLEARTID,
        CLONE_CHILD_SETTID,
    },
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
/// MIPS O32 ABI: $sp holds the stack pointer, $a0-$a3 are argument registers.
/// At entry, argc is at 0($sp), argv at 4($sp), etc.
// MIPS uses __start as the default entry point, not _start.
#[cfg(feature = "origin-start")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub(super) unsafe extern "C" fn __start() -> ! {
    // Jump to `entry`, passing it the initial stack pointer value as an
    // argument, a null return address, a null frame pointer, and an aligned
    // stack pointer.
    //
    // MIPS O32: $sp points to argc on the stack. We pass sp in $a0.
    // O32 ABI requires 16 bytes of stack space for argument save area.
    core::arch::naked_asm!(
        ".set noreorder",
        "move $a0, $sp",        // Pass the incoming `sp` as the arg to `entry`.
        "move $fp, $zero",      // Set the frame pointer to zero.
        "and $sp, $sp, -8",     // Align stack to 8 bytes.
        "subu $sp, $sp, 16",    // Reserve 16 bytes for O32 ABI argument save area.
        "move $ra, $zero",      // Set the return address to zero.
        "la $t9, {entry}",      // Load entry address into $t9.
        "jr $t9",               // Jump to `entry` via $t9 (required for PIC).
        "nop",                  // Branch delay slot.
        ".set reorder",
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
        // The `break` instruction causes a breakpoint exception on MIPS.
        asm!("break", options(noreturn, nostack));
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
            ".set noreorder",
            ".weak _DYNAMIC",
            ".hidden _DYNAMIC",
            "lui {0}, %hi(_DYNAMIC)",
            "addiu {0}, {0}, %lo(_DYNAMIC)",
            ".set reorder",
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
            ".set noreorder",
            "lui {0}, %hi(__ehdr_start)",
            "addiu {0}, {0}, %lo(__ehdr_start)",
            ".set reorder",
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
            "lw {0}, 0({1})",
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
            "lw {0}, 0({1})",
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
            "sw {0}, 0({1})",
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
        let err: usize;

        // MIPS O32 syscall: $v0 = syscall number, $a0-$a3 = args.
        // Return in $v0, $a3 = 0 on success, 1 on error.
        asm!(
            ".set noreorder",
            "syscall",
            "move {0}, $a3",
            ".set reorder",
            out(reg) err,
            in("$v0") __NR_mprotect,
            in("$a0") ptr,
            in("$a1") len,
            in("$a2") PROT_READ,
            lateout("$v0") _,
            lateout("$a3") _,
            lateout("$t0") _,
            lateout("$t1") _,
            lateout("$t2") _,
            lateout("$t3") _,
            lateout("$t4") _,
            lateout("$t5") _,
            lateout("$t6") _,
            lateout("$t7") _,
            lateout("$t8") _,
            lateout("$t9") _,
            options(nostack),
        );

        if err != 0 {
            // Do not panic here as libstd's panic handler needs TLS, which is not
            // yet initialized at this point.
            trap();
        }
    }
}

/// The required alignment for the stack pointer.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
pub(super) const STACK_ALIGNMENT: usize = 8;

// MIPS errno value for "operation not supported" (different from x86's 95).
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
const EOPNOTSUPP: i32 = 122;

/// Linux `clone` syscall wrapper. Inline asm required because child resumes
/// at same point as parent and must jump to our thread entrypoint.
///
/// CLONE_CHILD_CLEARTID/SETTID unsupported (O32 ABI needs child_tid on stack).
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) unsafe fn clone(
    flags: u32,
    child_stack: *mut c_void,
    parent_tid: *mut RawPid,
    _child_tid: *mut RawPid, // unused: O32 ABI needs stack passing, not implemented
    newtls: *mut c_void,
    fn_: extern "C" fn(),
    num_args: usize,
) -> isize {
    // Fail explicitly for flags that require child_tid, which we don't pass.
    if flags & (CLONE_CHILD_CLEARTID | CLONE_CHILD_SETTID) != 0 {
        return -(EOPNOTSUPP as isize);
    }

    unsafe {
        let r0;
        asm!(
            ".set noreorder",
            "syscall",            // Do the `clone` system call.
            "bnez $v0, 2f",       // Branch if we're in the parent thread ($v0 != 0).
            "nop",                // Delay slot.

            // Child thread.
            "move $a0, {fn_}",    // Pass `fn_` as the first argument.
            "move $a1, $sp",      // Pass the stack pointer as the second argument.
            "move $a2, {num_args}", // Pass `num_args` as the third argument.
            "move $fp, $zero",    // Zero the frame pointer.
            "move $ra, $zero",    // Zero the return address.
            "j {entry}",          // Jump to `entry`.
            "nop",                // Delay slot.

            // Parent thread.
            "2:",
            "move {0}, $v0",      // Copy return value.
            ".set reorder",

            out(reg) r0,
            entry = sym super::thread::entry,
            fn_ = in(reg) fn_,
            num_args = in(reg) num_args,
            in("$v0") __NR_clone,
            in("$a0") flags,
            in("$a1") child_stack,
            in("$a2") parent_tid,
            in("$a3") newtls,
            // child_tid goes on the stack for O32 ABI (5th arg)
            lateout("$a3") _,
            lateout("$t0") _,
            lateout("$t1") _,
            lateout("$t2") _,
            lateout("$t3") _,
            lateout("$t4") _,
            lateout("$t5") _,
            lateout("$t6") _,
            lateout("$t7") _,
            lateout("$t8") _,
            lateout("$t9") _,
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
    // MIPS uses the `rdhwr` instruction to read the thread pointer from
    // hardware register 29, but setting it requires a syscall.
    // For user-space TLS, we use the set_thread_area syscall.
    unsafe {
        let _: usize;
        asm!(
            "move $a0, {0}",
            ".set noreorder",
            "li $v0, {__NR_set_thread_area}",
            "syscall",
            ".set reorder",
            __NR_set_thread_area = const __NR_set_thread_area,
            in(reg) ptr,
            out("$v0") _,
            out("$a0") _,
            lateout("$a3") _,
            options(nostack)
        );
        debug_assert_eq!(thread_pointer(), ptr);
    }
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) fn thread_pointer() -> *mut c_void {
    let ptr;
    // SAFETY: Read the thread pointer using the rdhwr instruction.
    // Hardware register 29 contains the thread pointer.
    unsafe {
        asm!(
            ".set push",
            ".set mips32r2",
            "rdhwr {0}, $29",
            ".set pop",
            out(reg) ptr,
            options(nostack, preserves_flags, readonly)
        );
    }
    ptr
}

/// TLS data offset from the thread pointer on MIPS.
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
            ".set noreorder",
            "syscall",              // munmap syscall
            "move $a0, $zero",      // exit code 0
            "li $v0, {__NR_exit}",  // exit syscall number
            "syscall",              // exit syscall
            "break",                // should never reach here
            ".set reorder",
            __NR_exit = const __NR_exit,
            in("$v0") __NR_munmap,
            in("$a0") map_addr,
            in("$a1") map_len,
            options(noreturn, nostack)
        );
    }
}

// MIPS doesn't use sa_restorer for signal handling

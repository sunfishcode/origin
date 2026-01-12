//! Architecture-specific assembly code.

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
#[cfg(feature = "origin-start")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub(super) unsafe extern "C" fn _start() -> ! {
    // Jump to `entry`, passing it the initial stack pointer value as an
    // argument, a null return address, a null frame pointer, and an aligned
    // stack pointer. On many architectures, the incoming frame pointer is
    // already null.
    core::arch::naked_asm!(
        "mv a0, sp",    // Pass the incoming `sp` as the arg to `entry`.
        "mv ra, zero",  // Set the return address to zero.
        "mv fp, zero",  // Set the frame address to zero.
        "tail {entry}", // Jump to `entry`.
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
        asm!("unimp", options(noreturn, nostack));
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
            "lla {}, _DYNAMIC",
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
            "lla {}, __ehdr_start",
            out(reg) addr)
    };
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
            "ld {}, 0({})",
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
            "ld {}, 0({})",
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
            "sd {}, 0({})",
            in(reg) value,
            in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
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
    unsafe {
        let r0: usize;

        // This is read-only but we don't use `readonly` because the side effects
        // happen outside the Rust memory model. As far as Rust knows, this is
        // just an arbitrary side-effecting opaque operation.
        asm!(
            "ecall",
            in("a7") __NR_mprotect,
            inlateout("a0") ptr as usize => r0,
            in("a1") len,
            in("a2") PROT_READ,
            options(nostack, preserves_flags),
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
            "ecall",              // Do the `clone` system call.
            "bnez a0, 0f",        // Branch if we're in the parent thread.

            // Child thread.
            "mv a0, {fn_}",       // Pass `fn_` as the first argument.
            "mv a1, sp",          // Pass the args pointer as the second argument.
            "mv a2, {num_args}",  // Pass `num_args` as the third argument.
            "mv fp, zero",        // Zero the frame address.
            "mv ra, zero",        // Zero the return address.
            "tail {entry}",       // Call `entry`.

            // Parent thread.
            "0:",

            entry = sym super::thread::entry,
            fn_ = in(reg) fn_,
            num_args = in(reg) num_args,
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
}

/// Write a value to the platform thread-pointer register.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) unsafe fn set_thread_pointer(ptr: *mut c_void) {
    unsafe {
        asm!("mv tp, {}", in(reg) ptr);
        debug_assert_eq!(thread_pointer(), ptr);
    }
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) fn thread_pointer() -> *mut c_void {
    let ptr;
    // SAFETY: This reads the thread register.
    unsafe {
        asm!("mv {}, tp", out(reg) ptr, options(nostack, preserves_flags, readonly));
    }
    ptr
}

/// TLS data starts 0x800 bytes below the location pointed to by the thread
/// pointer.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
pub(super) const TLS_OFFSET: usize = 0x800;

/// `munmap` the current thread, then carefully exit the thread without
/// touching the deallocated stack.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) unsafe fn munmap_and_exit_thread(map_addr: *mut c_void, map_len: usize) -> ! {
    unsafe {
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
}

// RISC-V doesn't use `__NR_rt_sigreturn`

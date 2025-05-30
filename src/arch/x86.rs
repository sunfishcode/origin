//! Architecture-specific assembly code.

#[cfg(any(
    feature = "take-charge",
    all(not(feature = "unwinding"), feature = "panic-handler-trap")
))]
use core::arch::asm;
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
use core::ptr::without_provenance_mut;
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
use linux_raw_sys::elf::Elf_Ehdr;
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
use linux_raw_sys::general::{__NR_mprotect, PROT_READ};
#[cfg(feature = "take-charge")]
#[cfg(feature = "signal")]
use linux_raw_sys::general::{__NR_rt_sigreturn, __NR_sigreturn};
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
    "mov eax, esp", // Save the incoming `esp` value.
    "push ebp",     // Pad for stack pointer alignment.
    "push ebp",     // Pad for stack pointer alignment.
    "push ebp",     // Pad for stack pointer alignment.
    "push eax",     // Pass saved the incoming `esp` as the arg to `entry`.
    "push ebp",     // Set the return address to zero.
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
    // We ought to add `.cfi_adjust_cfa_offset` directives here describe our
    // modifications to the stack pointer value, however the Rust compiler
    // sometimes omits `.cfi_start_proc`/`.cfi_end_proc`, and when it does
    // this it prohibits use of `.cfi_adjust_cfa_offset`, and there doesn't
    // appear to be any way for us to test when it does this. So just comment
    // them out entirely for now.
    unsafe {
        asm!(
            ".weak _DYNAMIC",
            ".hidden _DYNAMIC",
            "call 2f",
            //".cfi_adjust_cfa_offset 4",
            "2:",
            "pop {0}", // We depend on this being exactly one byte long.
            //".cfi_adjust_cfa_offset -4",
            "add {0}, offset _GLOBAL_OFFSET_TABLE_+1",
            "lea {0}, [{0} + _DYNAMIC@GOTOFF]",
            out(reg) addr
        )
    }
    addr
}

/// Compute the dynamic address of `__ehdr_start`.
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
pub(super) fn ehdr_addr() -> *const Elf_Ehdr {
    let addr;
    unsafe {
        asm!(
            "call 2f",
            ".cfi_adjust_cfa_offset 4",
            "2:",
            "pop {0}",
            ".cfi_adjust_cfa_offset -4",
            "3:",
            // See the comment by similar code in `dynamic_table_addr`.
            ".ifne (3b-2b)-1",
            ".error \"The pop opcode is expected to be 1 byte long.\"",
            ".endif",
            "add {0}, offset _GLOBAL_OFFSET_TABLE_+1",
            "lea {0}, [{0} + __ehdr_start@GOTOFF]",
            out(reg) addr
        )
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
            "mov {}, [{}]",
            out(reg) r0,
            in(reg) ptr,
            options(nostack, preserves_flags),
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
            "mov [{}], {}",
            in(reg) ptr,
            in(reg) value,
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
            "int 0x80",
            inlateout("eax") __NR_mprotect as usize => r0,
            in("ebx") ptr,
            in("ecx") len,
            in("edx") PROT_READ,
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
        let mut gs: u32 = 0;
        asm!("mov {0:x}, gs", inout(reg) gs);
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

        // Push `fn_` to the child stack, since after all the `clone` args, and
        // `num_args` in `ebp`, there are no more free registers.
        let child_stack = child_stack.cast::<*mut c_void>().sub(1);
        child_stack.write(fn_ as _);

        // See the comments for x86's `syscall6` in `rustix`. Inline asm isn't
        // allowed to name ebp or esi as operands, so we have to jump through
        // extra hoops here.
        let r0;
        asm!(
            "push esi",           // Save incoming register value.
            "push ebp",           // Save incoming register value.

            // Pass `num_args` to the child in `ebp`.
            "mov ebp, [eax+8]",

            "mov esi, [eax+0]",   // Pass `newtls` to the `int 0x80`.
            "mov eax, [eax+4]",   // Pass `__NR_clone` to the `int 0x80`.

            // Use `int 0x80` instead of vsyscall, following `clone`'s
            // documentation; vsyscall would attempt to return to the parent stack
            // in the child.
            "int 0x80",           // Do the `clone` system call.
            "test eax, eax",      // Branch if we're in the parent.
            "jnz 2f",

            // Child thread.
            "pop edi",            // Load `fn_` from the child stack.
            "mov esi, esp",       // Snapshot the args pointer.
            "push eax",           // Pad for stack pointer alignment.
            "push ebp",           // Pass `num_args` as the third argument.
            "push esi",           // Pass the args pointer as the second argument.
            "push edi",           // Pass `fn_` as the first argument.
            "xor ebp, ebp",       // Zero the frame address.
            "push eax",           // Zero the return address.
            "jmp {entry}",        // Call `entry`.

            // Parent thread.
            "2:",
            "pop ebp",            // Restore incoming register value.
            "pop esi",            // Restore incoming register value.

            entry = sym super::thread::entry,
            inout("eax") &[
                newtls.cast::<c_void>().cast_mut(),
                without_provenance_mut(__NR_clone as usize),
                without_provenance_mut(num_args)
            ] => r0,
            in("ebx") flags,
            in("ecx") child_stack,
            in("edx") parent_tid,
            in("edi") child_tid,
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
        let res = rustix::runtime::set_thread_area(&mut user_desc);
        debug_assert_eq!(res, Ok(()));
        debug_assert_ne!(user_desc.entry_number, !0);

        asm!("mov gs, {0:x}", in(reg) ((user_desc.entry_number << 3) | 3) as u16);
        debug_assert_eq!(*ptr.cast::<*const c_void>(), ptr);
        debug_assert_eq!(thread_pointer(), ptr);
    }
}

/// Read the value of the platform thread-pointer register.
#[cfg(feature = "take-charge")]
#[cfg(feature = "thread")]
#[inline]
pub(super) fn thread_pointer() -> *mut c_void {
    let ptr;
    // SAFETY: On x86, reading the thread register itself is expensive, so the
    // ABI specifies that the thread pointer value is also stored in memory at
    // offset 0 from the thread pointer value, where it can be read with just a
    // load.
    unsafe {
        asm!("mov {}, gs:0", out(reg) ptr, options(nostack, preserves_flags, readonly));
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
    unsafe {
        asm!(
            // Use `int 0x80` instead of vsyscall, since vsyscall would attempt to
            // touch the stack after we `munmap` it.
            "int 0x80",
            "xor ebx, ebx",
            "mov eax, {__NR_exit}",
            "int 0x80",
            "ud2",
            __NR_exit = const __NR_exit,
            in("eax") __NR_munmap,
            in("ebx") map_addr,
            in("ecx") map_len,
            options(noreturn, nostack)
        );
    }
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

    "mov eax,  {__NR_rt_sigreturn}",
    "int 0x80",
    "ud2";
    __NR_rt_sigreturn = const __NR_rt_sigreturn
);

#[cfg(feature = "take-charge")]
#[cfg(feature = "signal")]
naked_fn!(
    "
    Invoke the appropriate system call to return control from a signal
    handler that does not use `SA_SIGINFO`.

    # Safety

    This function must never be called other than by the `sa_restorer`
    mechanism.
    ";
    pub(super) fn return_from_signal_handler_noinfo() -> ();

    "pop eax",
    "mov eax, {__NR_sigreturn}",
    "int 0x80",
    "ud2";
    __NR_sigreturn = const __NR_sigreturn
);

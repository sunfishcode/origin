#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(link_llvm_intrinsics)]
#![feature(strict_provenance)]
#![deny(fuzzy_provenance_casts)]
#![deny(lossy_provenance_casts)]
#![no_std]

#[cfg(all(feature = "alloc", not(feature = "rustc-dep-of-std")))]
extern crate alloc;

#[cfg(not(target_arch = "arm"))]
extern crate unwinding;

pub mod program;
#[cfg(feature = "signal")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "signal")))]
#[cfg_attr(not(feature = "origin-signal"), path = "signal_via_libc.rs")]
pub mod signal;
#[cfg(feature = "thread")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "thread")))]
#[cfg_attr(not(feature = "origin-thread"), path = "thread_via_libpthread.rs")]
pub mod thread;

// Unwinding isn't supported on 32-bit arm yet.
#[cfg(target_arch = "arm")]
mod unwind;

#[cfg(any(feature = "origin-thread", feature = "origin-signal"))]
#[cfg_attr(target_arch = "aarch64", path = "arch-aarch64.rs")]
#[cfg_attr(target_arch = "x86_64", path = "arch-x86_64.rs")]
#[cfg_attr(target_arch = "x86", path = "arch-x86.rs")]
#[cfg_attr(target_arch = "riscv64", path = "arch-riscv64.rs")]
#[cfg_attr(target_arch = "arm", path = "arch-arm.rs")]
mod arch;

/// The program entry point.
///
/// # Safety
///
/// This function should never be called explicitly. It is the first thing
/// executed in the program, and it assumes that memory is laid out according
/// to the operating system convention for starting a new program.
#[cfg(feature = "origin-program")]
#[cfg(feature = "origin-start")]
#[naked]
#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    use core::arch::asm;
    use program::entry;

    // Jump to `entry`, passing it the initial stack pointer value as an
    // argument, a null return address, a null frame pointer, and an aligned
    // stack pointer. On many architectures, the incoming frame pointer is
    // already null.

    #[cfg(target_arch = "x86_64")]
    asm!(
        "mov rdi, rsp", // Pass the incoming `rsp` as the arg to `entry`.
        "push rbp",     // Set the return address to zero.
        "jmp {entry}",  // Jump to `entry`.
        entry = sym entry,
        options(noreturn),
    );

    #[cfg(target_arch = "aarch64")]
    asm!(
        "mov x0, sp",   // Pass the incoming `sp` as the arg to `entry`.
        "mov x30, xzr", // Set the return address to zero.
        "b {entry}",    // Jump to `entry`.
        entry = sym entry,
        options(noreturn),
    );

    #[cfg(target_arch = "arm")]
    asm!(
        "mov r0, sp\n", // Pass the incoming `sp` as the arg to `entry`.
        "mov lr, #0",   // Set the return address to zero.
        "b {entry}",    // Jump to `entry`.
        entry = sym entry,
        options(noreturn),
    );

    #[cfg(target_arch = "riscv64")]
    asm!(
        "mv a0, sp",    // Pass the incoming `sp` as the arg to `entry`.
        "mv ra, zero",  // Set the return address to zero.
        "mv fp, zero",  // Set the frame address to zero.
        "tail {entry}", // Jump to `entry`.
        entry = sym entry,
        options(noreturn),
    );

    #[cfg(target_arch = "x86")]
    asm!(
        "mov eax, esp", // Save the incoming `esp` value.
        "push ebp",     // Pad for alignment.
        "push ebp",     // Pad for alignment.
        "push ebp",     // Pad for alignment.
        "push eax",     // Pass saved the incoming `esp` as the arg to `entry`.
        "push ebp",     // Set the return address to zero.
        "jmp {entry}",  // Jump to `entry`.
        entry = sym entry,
        options(noreturn),
    );
}

/// A program entry point similar to `_start`, but which is meant to be called
/// by something else in the program rather than the OS.
///
/// # Safety
///
/// `mem` must point to a stack with the contents that the OS would provide
/// on the initial stack.
#[cfg(feature = "origin-program")]
#[cfg(feature = "external-start")]
pub unsafe fn start(mem: *mut usize) -> ! {
    program::entry(mem)
}

/// An ABI-conforming `__dso_handle`.
#[cfg(feature = "origin-program")]
#[cfg(feature = "origin-start")]
#[no_mangle]
static __dso_handle: UnsafeSendSyncVoidStar =
    UnsafeSendSyncVoidStar(&__dso_handle as *const _ as *const _);

/// A type for `__dso_handle`.
///
/// `*const c_void` isn't `Send` or `Sync` because a raw pointer could point to
/// arbitrary data which isn't thread-safe, however `__dso_handle` is used as
/// an opaque cookie value, and it always points to itself.
///
/// Note that in C, `__dso_handle`'s type is usually `void *` which would
/// correspond to `*mut c_void`, however we can assume the pointee is never
/// actually mutated.
#[cfg(feature = "origin-program")]
#[repr(transparent)]
struct UnsafeSendSyncVoidStar(*const core::ffi::c_void);
#[cfg(feature = "origin-program")]
unsafe impl Send for UnsafeSendSyncVoidStar {}
#[cfg(feature = "origin-program")]
unsafe impl Sync for UnsafeSendSyncVoidStar {}

/// Initialize logging, if enabled.
#[cfg(feature = "log")]
#[link_section = ".init_array.00099"]
#[used]
static INIT_ARRAY: unsafe extern "C" fn() = {
    unsafe extern "C" fn function() {
        #[cfg(feature = "env_logger")]
        env_logger::init();

        log::trace!(target: "origin::program", "Program started");

        // Log the thread id. We initialized the main earlier than this, but
        // we couldn't initialize the logger until after the main thread is
        // intialized :-).
        #[cfg(feature = "origin-thread")]
        log::trace!(target: "origin::thread", "Main Thread[{:?}] initialized", current_thread_id());
    }
    function
};

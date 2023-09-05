#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(link_llvm_intrinsics)]
#![cfg_attr(feature = "experimental-relocate", feature(cfg_relocation_model))]
#![feature(strict_provenance)]
#![deny(fuzzy_provenance_casts)]
#![deny(lossy_provenance_casts)]
#![no_std]

#![allow(dead_code)] // FIXME: this is just so tests pass while I implement everything


#[cfg(all(feature = "alloc", not(feature = "rustc-dep-of-std")))]
extern crate alloc;

// Pull in the `unwinding` crate to satisfy `_Unwind_* symbol references.
// Except that 32-bit arm isn't supported yet, so we use stubs instead.
#[cfg(not(target_arch = "arm"))]
extern crate unwinding;
#[cfg(target_arch = "arm")]
mod unwind;

pub mod program;
#[cfg(feature = "signal")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "signal")))]
#[cfg_attr(feature = "origin-signal", path = "signal/linux_raw.rs")]
#[cfg_attr(not(feature = "origin-signal"), path = "signal/libc.rs")]
pub mod signal;
#[cfg(feature = "thread")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "thread")))]
#[cfg_attr(feature = "origin-thread", path = "thread/linux_raw.rs")]
#[cfg_attr(not(feature = "origin-thread"), path = "thread/libc.rs")]
pub mod thread;

#[cfg(any(feature = "origin-thread", feature = "origin-signal"))]
#[cfg_attr(target_arch = "aarch64", path = "arch/aarch64.rs")]
#[cfg_attr(target_arch = "x86_64", path = "arch/x86_64.rs")]
#[cfg_attr(target_arch = "x86", path = "arch/x86.rs")]
#[cfg_attr(target_arch = "riscv64", path = "arch/riscv64.rs")]
#[cfg_attr(target_arch = "arm", path = "arch/arm.rs")]
mod arch;

#[cfg(feature = "macros")]
pub use origin_macros::main;

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
        #[cfg(feature = "atomic-dbg")]
        atomic_dbg::log::init();
        #[cfg(feature = "env_logger")]
        env_logger::init();

        log::trace!(target: "origin::program", "Program started");

        // Log the thread id. We initialized the main earlier than this, but
        // we couldn't initialize the logger until after the main thread is
        // intialized :-).
        #[cfg(feature = "origin-thread")]
        log::trace!(
            target: "origin::thread",
            "Main Thread[{:?}] initialized",
            thread::current_thread_id()
        );
    }
    function
};

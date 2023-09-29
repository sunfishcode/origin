#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(debug_assertions, allow(internal_features))]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![feature(asm_const)]
#![feature(naked_functions)]
#![cfg_attr(debug_assertions, feature(link_llvm_intrinsics))]
#![cfg_attr(feature = "experimental-relocate", feature(cfg_relocation_model))]
#![feature(strict_provenance)]
#![deny(fuzzy_provenance_casts, lossy_provenance_casts)]
#![no_std]

#[cfg(all(feature = "alloc", not(feature = "rustc-dep-of-std")))]
extern crate alloc;

// Pull in the `unwinding` crate to satisfy `_Unwind_*` symbol references.
// Except that 32-bit arm isn't supported yet, so we use stubs instead.
#[cfg(not(target_arch = "arm"))]
extern crate unwinding;
#[cfg(target_arch = "arm")]
mod unwind_unimplemented;

#[cfg(any(
    feature = "origin-thread",
    feature = "origin-signal",
    all(feature = "origin-program", feature = "origin-start")
))]
#[cfg_attr(target_arch = "aarch64", path = "arch/aarch64.rs")]
#[cfg_attr(target_arch = "x86_64", path = "arch/x86_64.rs")]
#[cfg_attr(target_arch = "x86", path = "arch/x86.rs")]
#[cfg_attr(target_arch = "riscv64", path = "arch/riscv64.rs")]
#[cfg_attr(target_arch = "arm", path = "arch/arm.rs")]
mod arch;
#[cfg(feature = "log")]
mod log;

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

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(debug_assertions, allow(internal_features))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![feature(asm_const)]
#![feature(naked_functions)]
#![cfg_attr(debug_assertions, feature(link_llvm_intrinsics))]
#![cfg_attr(feature = "experimental-relocate", feature(cfg_relocation_model))]
#![feature(strict_provenance)]
#![feature(exposed_provenance)]
#![deny(fuzzy_provenance_casts, lossy_provenance_casts)]
#![no_std]

#[cfg(all(feature = "alloc", not(feature = "rustc-dep-of-std")))]
extern crate alloc;

// Pull in the `unwinding` crate to satisfy `_Unwind_*` symbol references.
// Except that 32-bit arm isn't supported yet, so we use stubs instead.
#[cfg(not(target_arch = "arm"))]
#[allow(unused_extern_crates)]
extern crate unwinding;
#[cfg(target_arch = "arm")]
mod unwind_unimplemented;

#[cfg(feature = "take-charge")]
#[cfg_attr(target_arch = "aarch64", path = "arch/aarch64.rs")]
#[cfg_attr(target_arch = "x86_64", path = "arch/x86_64.rs")]
#[cfg_attr(target_arch = "x86", path = "arch/x86.rs")]
#[cfg_attr(target_arch = "riscv64", path = "arch/riscv64.rs")]
#[cfg_attr(target_arch = "arm", path = "arch/arm.rs")]
mod arch;
#[cfg(all(feature = "take-charge", feature = "log"))]
mod log;
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
mod relocate;

#[cfg_attr(feature = "take-charge", path = "program/linux_raw.rs")]
#[cfg_attr(not(feature = "take-charge"), path = "program/libc.rs")]
pub mod program;
#[cfg(feature = "signal")]
#[cfg_attr(docsrs, doc(cfg(feature = "signal")))]
#[cfg_attr(feature = "take-charge", path = "signal/linux_raw.rs")]
#[cfg_attr(not(feature = "take-charge"), path = "signal/libc.rs")]
pub mod signal;
#[cfg(feature = "thread")]
#[cfg_attr(docsrs, doc(cfg(feature = "thread")))]
#[cfg_attr(feature = "take-charge", path = "thread/linux_raw.rs")]
#[cfg_attr(not(feature = "take-charge"), path = "thread/libc.rs")]
pub mod thread;

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(feature = "experimental-relocate", feature(cfg_relocation_model))]
// On nightly, enable `#[naked]` functions.
#![cfg_attr(feature = "nightly", feature(naked_functions))]
// On nightly, enable llvm intrinsics for additional debug asserts.
#![cfg_attr(all(debug_assertions, feature = "nightly"), allow(internal_features))]
#![cfg_attr(
    all(debug_assertions, feature = "nightly"),
    feature(link_llvm_intrinsics)
)]
// Allow our polyfills to polyfill nightly features.
#![cfg_attr(not(feature = "nightly"), allow(unstable_name_collisions))]
// On nightly, enable strict provenance.
#![cfg_attr(feature = "nightly", feature(strict_provenance))]
#![cfg_attr(feature = "nightly", feature(exposed_provenance))]
#![cfg_attr(
    feature = "nightly",
    deny(fuzzy_provenance_casts, lossy_provenance_casts)
)]

#[cfg(all(feature = "alloc", not(feature = "rustc-dep-of-std")))]
extern crate alloc;

// Strict-provenance API polyfill.
#[cfg(not(feature = "nightly"))]
pub(crate) mod ptr;

// Wrapper/polyfill for `#[naked]`.
#[macro_use]
pub(crate) mod naked;

// Pull in the `unwinding` crate to satisfy `_Unwind_*` symbol references.
// Except that 32-bit arm isn't supported yet, so we use stubs instead.
#[cfg(all(feature = "unwinding", not(target_arch = "arm")))]
#[allow(unused_extern_crates)]
extern crate unwinding;
#[cfg(all(feature = "unwinding", target_arch = "arm"))]
mod unwind_unimplemented;

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

// If we don't have "unwinding", provide stub functions for unwinding and
// panicking.
#[cfg(not(feature = "unwinding"))]
mod stubs;

// Include definitions of `memcpy` and other functions called from LLVM
// Codegen. Normally, these would be defined by the platform libc, however with
// origin with `take-charge`, there is no libc. Otherwise normally, these
// would be defined by the compiler-builtins crate, however compiler-builtins
// depends on nightly Rust and seems to be complex to use for this purpose.
// So instead, we have our own copy of the code from compiler-builtins for just
// these functions, so that they're always defined.
#[cfg(feature = "take-charge")]
mod mem;

// On aarch64, Rust's builtin compiler_builtins depends on `getauxval`.
#[cfg(any(feature = "getauxval", target_arch = "aarch64"))]
#[cfg(feature = "take-charge")]
mod getauxval;

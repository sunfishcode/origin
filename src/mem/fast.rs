//! `memcpy` etc. implementations with performance optimizations.
//!
//! The following is derived from src/mem/mod.rs in Rust's
//! [compiler_builtins library] at revision
//! cb060052ab7e4bad408c85d44be7e60096e93e38.
//!
//! [compiler_builtins library]: https://github.com/rust-lang/compiler-builtins

// memcpy/memmove/memset have optimized implementations on some architectures
#[cfg_attr(target_arch = "x86_64", path = "x86_64.rs")]
#[cfg_attr(not(target_arch = "x86_64"), path = "impls.rs")]
mod impls;

#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 { unsafe {
    impls::copy_forward(dest, src, n);
    dest
} }

#[unsafe(no_mangle)]
unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 { unsafe {
    let delta = dest.addr().wrapping_sub(src.addr());
    if delta >= n {
        // We can copy forwards because either dest is far enough ahead of src,
        // or src is ahead of dest (and delta overflowed).
        impls::copy_forward(dest, src, n);
    } else {
        impls::copy_backward(dest, src, n);
    }
    dest
} }

#[unsafe(no_mangle)]
unsafe extern "C" fn memset(s: *mut u8, c: core::ffi::c_int, n: usize) -> *mut u8 { unsafe {
    impls::set_bytes(s, c as u8, n);
    s
} }

#[unsafe(no_mangle)]
unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 { unsafe {
    impls::compare_bytes(s1, s2, n)
} }

#[unsafe(no_mangle)]
unsafe extern "C" fn bcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 { unsafe {
    memcmp(s1, s2, n)
} }

#[unsafe(no_mangle)]
unsafe extern "C" fn strlen(s: *const core::ffi::c_char) -> usize { unsafe {
    impls::c_string_length(s)
} }

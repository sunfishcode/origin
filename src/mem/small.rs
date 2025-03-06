//! `memcpy` etc. implementations with small code size.
//!
//! This code uses `core::arch::asm!("")` to try to discourage optimizers from
//! vectorizing or pattern-matching these loops.

use core::ffi::{c_char, c_int, c_void};

#[no_mangle]
unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, len: usize) -> *mut c_void {
    let start = dst;
    let mut dst = dst.cast::<u8>();
    let mut src = src.cast::<u8>();
    let dst_end = dst.add(len);
    while dst < dst_end {
        *dst = *src;
        dst = dst.add(1);
        src = src.add(1);
        core::arch::asm!("");
    }
    start
}

#[no_mangle]
unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, len: usize) -> *mut c_void {
    let start = dst;
    let mut dst = dst.cast::<u8>();
    let mut src = src.cast::<u8>();
    let delta = (dst.addr()).wrapping_sub(src.addr());
    if delta >= len {
        let dst_end = dst.add(len);
        while dst < dst_end {
            *dst = *src;
            dst = dst.add(1);
            src = src.add(1);
            core::arch::asm!("");
        }
    } else {
        let dst_start = dst;
        let mut dst = dst.add(len);
        let mut src = src.add(len);
        while dst > dst_start {
            dst = dst.sub(1);
            src = src.sub(1);
            *dst = *src;
            core::arch::asm!("");
        }
    }
    start
}

#[no_mangle]
unsafe extern "C" fn memset(dst: *mut c_void, fill: c_int, len: usize) -> *mut c_void {
    let mut s = dst.cast::<u8>();
    let end = s.add(len);
    while s < end {
        *s = fill as _;
        s = s.add(1);
        core::arch::asm!("");
    }
    dst
}

#[no_mangle]
unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, len: usize) -> c_int {
    let a = a.cast::<u8>();
    let b = b.cast::<u8>();
    let mut i = 0;
    while i < len {
        let a = *a.add(i);
        let b = *b.add(i);
        if a != b {
            return a as i32 - b as i32;
        }
        i += 1;
        core::arch::asm!("");
    }
    0
}

// Obsolescent
#[no_mangle]
unsafe extern "C" fn bcmp(a: *const c_void, b: *const c_void, len: usize) -> c_int {
    memcmp(a, b, len)
}

#[no_mangle]
unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    let mut s = s;
    let mut n = 0;
    while *s != 0 {
        n += 1;
        s = s.add(1);
        core::arch::asm!("");
    }
    n
}

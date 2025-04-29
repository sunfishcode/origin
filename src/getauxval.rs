//! Define a C-compatible `getauxv`.
//!
//! This may be needed to satisfy `compiler_builtins` or other low-level code.

use core::ffi::{c_ulong, c_void};
use core::ptr::without_provenance_mut;

// `getauxval` usually returns `unsigned long`, but we make it a pointer type
// so that it preserves provenance.
#[unsafe(no_mangle)]
unsafe extern "C" fn getauxval(type_: c_ulong) -> *mut c_void {
    _getauxval(type_)
}

#[cfg(target_arch = "aarch64")]
#[unsafe(no_mangle)]
unsafe extern "C" fn __getauxval(type_: c_ulong) -> *mut c_void {
    _getauxval(type_)
}

fn _getauxval(type_: c_ulong) -> *mut c_void {
    match type_ as _ {
        linux_raw_sys::general::AT_HWCAP => without_provenance_mut(rustix::param::linux_hwcap().0),
        linux_raw_sys::general::AT_HWCAP2 => without_provenance_mut(rustix::param::linux_hwcap().1),
        linux_raw_sys::general::AT_MINSIGSTKSZ => {
            without_provenance_mut(rustix::param::linux_minsigstksz())
        }
        _ => todo!("unrecognized __getauxval {}", type_),
    }
}

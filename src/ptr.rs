//! Polyfill for nightly Rust APIs.

#![allow(dead_code)]

#[inline]
pub(crate) const fn without_provenance_mut<T>(addr: usize) -> *mut T {
    // SAFETY: Every valid `usize` is also a valid pointer.
    unsafe { core::mem::transmute(addr) }
}

#[inline]
pub(crate) fn with_exposed_provenance_mut<T>(addr: usize) -> *mut T {
    addr as *mut T
}

/// Replacement for `.addr()` for the relocation code which can't call trait
/// methods because they might not be relocated yet.
#[cfg(feature = "experimental-relocate")]
#[inline]
pub(crate) fn addr<T>(addr: *const T) -> usize {
    // SAFETY: Every pointer is also a valid `usize`.
    unsafe { core::mem::transmute(addr) }
}

pub(crate) trait Polyfill<T> {
    fn addr(self) -> usize;
    fn expose_provenance(self) -> usize;
    fn with_addr(self, addr: usize) -> *mut T;
    fn map_addr(self, f: impl FnOnce(usize) -> usize) -> *mut T;
    fn cast_mut(self) -> *mut T;
}

impl<T> Polyfill<T> for *mut T {
    #[inline]
    fn addr(self) -> usize {
        // SAFETY: Every pointer is also a valid `usize`.
        unsafe { core::mem::transmute(self) }
    }

    #[inline]
    fn expose_provenance(self) -> usize {
        self as usize
    }

    #[inline]
    fn with_addr(self, addr: usize) -> *mut T {
        self.wrapping_byte_offset((addr as isize).wrapping_sub(self.addr() as isize))
    }

    #[inline]
    fn map_addr(self, f: impl FnOnce(usize) -> usize) -> *mut T {
        self.with_addr(f(self.addr()))
    }

    #[inline]
    fn cast_mut(self) -> *mut T {
        self
    }
}

impl<T> Polyfill<T> for *const T {
    #[inline]
    fn addr(self) -> usize {
        // SAFETY: Every pointer is also a valid `usize`.
        unsafe { core::mem::transmute(self) }
    }

    #[inline]
    fn expose_provenance(self) -> usize {
        self as usize
    }

    #[inline]
    fn with_addr(self, addr: usize) -> *mut T {
        self.wrapping_byte_offset((addr as isize).wrapping_sub(self.addr() as isize)) as *mut T
    }

    #[inline]
    fn map_addr(self, f: impl FnOnce(usize) -> usize) -> *mut T {
        self.with_addr(f(self.addr())).cast_mut()
    }

    #[inline]
    fn cast_mut(self) -> *mut T {
        self as *mut T
    }
}

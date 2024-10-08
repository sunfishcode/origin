//! If we don't have "nightly", we don't have the unwinding crate, so provide
//! stub functions for unwinding and panicking.

// If requested, provide a version of "eh-personality-continue" ourselves.
#[cfg(feature = "eh-personality-continue")]
#[no_mangle]
unsafe extern "C" fn rust_eh_personality(
    version: core::ffi::c_int,
    _actions: core::ffi::c_int,
    _exception_class: u64,
    _exception: *mut core::ffi::c_void,
    _ctx: *mut core::ffi::c_void,
) -> core::ffi::c_int {
    if version != 1 {
        return 3; // UnwindReasonCode::FATAL_PHASE1_ERROR
    }
    8 // UnwindReasonCode::CONTINUE_UNWIND
}

// If requested, provide a version of "panic-handler-abort" ourselves.
#[cfg(feature = "panic-handler-abort")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    crate::arch::abort()
}

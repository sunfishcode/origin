//! Stub libunwind implementation on platforms where we don't have real unwind
//! support.
//!
//! Entirely `unimplemented!`.

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_Backtrace() {
    unimplemented!("_Unwind_Backtrace")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_DeleteException() {
    unimplemented!("_Unwind_DeleteException")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_GetDataRelBase() {
    unimplemented!("_Unwind_GetDataRelBase")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_GetIP() {
    unimplemented!("_Unwind_GetIP")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_GetIPInfo() {
    unimplemented!("_Unwind_GetIPInfo")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_GetLanguageSpecificData() {
    unimplemented!("_Unwind_GetLanguageSpecificData")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_GetRegionStart() {
    unimplemented!("_Unwind_GetRegionStart")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_GetTextRelBase() {
    unimplemented!("_Unwind_GetTextRelBase")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_RaiseException() {
    unimplemented!("_Unwind_RaiseException")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_Resume() {
    unimplemented!("_Unwind_Resume")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_SetGR() {
    unimplemented!("_Unwind_SetGR")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_SetIP() {
    unimplemented!("_Unwind_SetIP")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_FindEnclosingFunction() {
    unimplemented!("_Unwind_FindEnclosingFunction")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _Unwind_GetCFA() {
    unimplemented!("_Unwind_GetCFA")
}

#[cfg(target_arch = "arm")]
#[no_mangle]
unsafe extern "C" fn _Unwind_VRS_Get() {
    unimplemented!("_Unwind_VRS_Get")
}

#[cfg(target_arch = "arm")]
#[no_mangle]
unsafe extern "C" fn _Unwind_VRS_Set() {
    unimplemented!("_Unwind_VRS_Set")
}

#[cfg(target_arch = "arm")]
#[no_mangle]
unsafe extern "C" fn __aeabi_unwind_cpp_pr0() {
    unimplemented!("__aeabi_unwind_cpp_pr0")
}

#[cfg(target_arch = "arm")]
#[no_mangle]
unsafe extern "C" fn __aeabi_unwind_cpp_pr1() {
    unimplemented!("__aeabi_unwind_cpp_pr1")
}

#[cfg(target_arch = "arm")]
#[no_mangle]
unsafe extern "C" fn __gnu_unwind_frame() {
    unimplemented!("__gnu_unwind_frame")
}

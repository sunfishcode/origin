//! Test that workarounds for missing `asm_const` have the right values..
//!
//! Due to `asm_const` not being stable yet, src/arch/* currently have to
//! hard-code these ABI magic numbers. Test that the hard-coded numbers
//! match the upstream numbers.
//!
//! Keep these in sync with the code in src/arch/*.
//!
//! TODO: When `asm_const` is stable, obviate these tests.

#[cfg(target_arch = "x86")]
#[test]
fn test_rt_sigreturn() {
    assert_eq!(linux_raw_sys::general::__NR_rt_sigreturn, 173);
}

#[cfg(target_arch = "x86")]
#[test]
fn test_sigreturn() {
    assert_eq!(linux_raw_sys::general::__NR_sigreturn, 119);
}

#[cfg(target_arch = "aarch64")]
#[test]
fn test_rt_sigreturn() {
    assert_eq!(linux_raw_sys::general::__NR_rt_sigreturn, 139);
}

#[cfg(target_arch = "arm")]
#[test]
fn test_rt_sigreturn() {
    assert_eq!(linux_raw_sys::general::__NR_rt_sigreturn, 173);
}

#[cfg(target_arch = "arm")]
#[test]
fn test_sigreturn() {
    assert_eq!(linux_raw_sys::general::__NR_sigreturn, 119);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn test_rt_sigreturn() {
    assert_eq!(linux_raw_sys::general::__NR_rt_sigreturn, 15);
}

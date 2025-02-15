//! Program startup and shutdown.
//!
//! To use origin's program startup enable the `take-charge` feature and define
//! a function named `origin_main` like this:
//!
//! ```no_run
//! /// This function is called by Origin.
//! ///
//! /// SAFETY: `argc`, `argv`, and `envp` describe incoming program
//! /// command-line arguments and environment variables.
//! #[no_mangle]
//! unsafe fn origin_main(argc: usize, argv: *mut *mut u8, envp: *mut *mut u8) -> i32 {
//!     todo!("Run the program and return the program exit status.")
//! }
//! ```
//!
//! Origin will call this function after starting up the program and running
//! the constructors when `take-charge` is enabled. `argc` is the number of
//! command-line arguments with a value of at most `c_int::MAX`, and `argv` is
//! a pointer to a NULL-terminated array of pointers to NUL-terminated C
//! strings. `argc` and `argv` describe the command-line arguments. `envp` is a
//! pointer to a NULL-terminated array of pointers to NUL-terminated C strings
//! containing a key followed by `b'='` followed by a value. It describes the
//! environment variables. The function should return a value for the program
//! exit status.
//!
//! This is a low-level and somewhat C-flavored interface, which is in tension
//! with origin's goal of providing Rust-idiomatic interfaces, however it does
//! mean that origin can avoid doing any work that users might not need.

#[cfg(feature = "program-at-exit")]
use alloc::boxed::Box;
#[cfg(feature = "program-at-exit")]
use core::ptr::null_mut;
use linux_raw_sys::ctypes::c_int;

/// Register a function to be called when [`exit`] is called.
#[cfg(feature = "program-at-exit")]
#[cfg_attr(docsrs, doc(cfg(feature = "program-at-exit")))]
pub fn at_exit(func: Box<dyn FnOnce() + Send>) {
    use core::ffi::c_void;

    extern "C" {
        // <https://refspecs.linuxbase.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/baselib---cxa-atexit.html>
        fn __cxa_atexit(
            func: unsafe extern "C" fn(*mut c_void),
            arg: *mut c_void,
            _dso: *mut c_void,
        ) -> c_int;
    }

    // The function to pass to `__cxa_atexit`.
    unsafe extern "C" fn at_exit_func(arg: *mut c_void) {
        Box::from_raw(arg.cast::<Box<dyn FnOnce() + Send>>())();
    }

    let at_exit_arg = Box::into_raw(Box::new(func)).cast::<c_void>();
    let r = unsafe { __cxa_atexit(at_exit_func, at_exit_arg, null_mut()) };
    assert_eq!(r, 0);
}

/// Call all the functions registered with [`at_exit`] or with the
/// `.fini_array` section, and exit the program.
pub fn exit(status: c_int) -> ! {
    unsafe {
        // Call `libc` to run *its* dtors, and exit the program.
        libc::exit(status)
    }
}

/// Exit the program without calling functions registered with [`at_exit`] or
/// with the `.fini_array` section.
#[inline]
pub fn immediate_exit(status: c_int) -> ! {
    unsafe {
        // Call `libc` to exit the program.
        libc::_exit(status)
    }
}

/// Execute a trap instruction.
///
/// This will produce a `Signal::Ill`, which by default will immediately
/// terminate the process.
#[inline]
#[cold]
pub fn trap() -> ! {
    // Emulate the effect of executing a trap instruction.
    loop {
        unsafe {
            libc::raise(libc::SIGILL);
        }
    }
}

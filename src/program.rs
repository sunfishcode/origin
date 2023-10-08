//! Program startup and shutdown.
//!
//! To use origin's program startup, define a function named `origin_main` like
//! this:
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
//! the constructors. `argc` is the number of command-line arguments with a
//! value of at most `c_int::MAX`, and `argv` is a pointer to a NULL-terminated
//! array of pointers to NUL-terminated C strings. `argc` and `argv` describe
//! the command-line arguments. `envp` is a pointer to a NULL-terminated array
//! of pointers to NUL-terminated C strings containing a key followed by `b'='`
//! followed by a value. It describes the environment variables. The function
//! should return a value for the program exit status.
//!
//! This is a low-level and somewhat C-flavored interface, which is in tension
//! with origin's goal of providing Rust-idiomatic interfaces, however it does
//! mean that origin can avoid doing any work that users might not need.

#[cfg(feature = "origin-thread")]
use crate::thread::{initialize_main_thread, initialize_startup_thread_info};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(all(feature = "alloc", feature = "origin-program"))]
use alloc::vec::Vec;
#[cfg(not(feature = "origin-program"))]
use core::ptr::null_mut;
use linux_raw_sys::ctypes::c_int;
#[cfg(all(feature = "alloc", feature = "origin-program"))]
use rustix_futex_sync::Mutex;

#[cfg(all(
    feature = "program-start",
    not(any(feature = "origin-start", feature = "external-start"))
))]
compile_error!("\"origin-program\" depends on either \"origin-start\" or \"external-start\".");

/// The entrypoint where Rust code is first executed when the program starts.
///
/// # Safety
///
/// `mem` must point to the stack as provided by the operating system.
#[cfg(feature = "origin-program")]
pub(super) unsafe extern "C" fn entry(mem: *mut usize) -> ! {
    // Do some basic precondition checks, to ensure that our assembly code did
    // what we expect it to do. These are debug-only, to keep the release-mode
    // startup code small and simple to disassemble and inspect.
    #[cfg(debug_assertions)]
    #[cfg(feature = "origin-start")]
    {
        extern "C" {
            #[link_name = "llvm.frameaddress"]
            fn builtin_frame_address(level: i32) -> *const u8;
            #[link_name = "llvm.returnaddress"]
            fn builtin_return_address(level: i32) -> *const u8;
            #[cfg(target_arch = "aarch64")]
            #[link_name = "llvm.sponentry"]
            fn builtin_sponentry() -> *const u8;
        }

        // Check that `mem` is where we expect it to be.
        debug_assert_ne!(mem, core::ptr::null_mut());
        debug_assert_eq!(mem.addr() & 0xf, 0);
        debug_assert!(builtin_frame_address(0).addr() <= mem.addr());

        // Check that the incoming stack pointer is where we expect it to be.
        debug_assert_eq!(builtin_return_address(0), core::ptr::null());
        debug_assert_ne!(builtin_frame_address(0), core::ptr::null());
        #[cfg(not(any(target_arch = "arm", target_arch = "x86")))]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0xf, 0);
        #[cfg(target_arch = "arm")]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0x7, 0);
        #[cfg(target_arch = "x86")]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0xf, 8);
        debug_assert_eq!(builtin_frame_address(1), core::ptr::null());
        #[cfg(target_arch = "aarch64")]
        debug_assert_ne!(builtin_sponentry(), core::ptr::null());
        #[cfg(target_arch = "aarch64")]
        debug_assert_eq!(builtin_sponentry().addr() & 0xf, 0);
    }

    // Compute `argc`, `argv`, and `envp`.
    let (argc, argv, envp) = compute_args(mem);

    // Initialize program state before running any user code.
    init_runtime(mem, envp);

    // Call the functions registered via `.init_array`.
    #[cfg(feature = "init-fini-arrays")]
    {
        use core::arch::asm;
        use core::ffi::c_void;

        // The linker-generated symbols that mark the start and end of the
        // `.init_array` section.
        extern "C" {
            static __init_array_start: c_void;
            static __init_array_end: c_void;
        }

        // Call the `.init_array` functions. As GLIBC does, pass argc, argv,
        // and envp as extra arguments. In addition to GLIBC ABI compatibility,
        // c-scape relies on this.
        type InitFn = extern "C" fn(c_int, *mut *mut u8, *mut *mut u8);
        let mut init = &__init_array_start as *const _ as *const InitFn;
        let init_end = &__init_array_end as *const _ as *const InitFn;
        // Prevent the optimizer from optimizing the `!=` comparison to true;
        // `init` and `init_start` may have the same address.
        asm!("# {}", inout(reg) init, options(pure, nomem, nostack, preserves_flags));

        while init != init_end {
            #[cfg(feature = "log")]
            log::trace!(
                "Calling `.init_array`-registered function `{:?}({:?}, {:?}, {:?})`",
                *init,
                argc,
                argv,
                envp
            );

            (*init)(argc, argv, envp);

            init = init.add(1);
        }
    }

    {
        // Declare `origin_main` as documented in [`crate::program`].
        extern "Rust" {
            fn origin_main(argc: usize, argv: *mut *mut u8, envp: *mut *mut u8) -> i32;
        }

        #[cfg(feature = "log")]
        log::trace!("Calling `origin_main({:?}, {:?}, {:?})`", argc, argv, envp);

        // Call `origin_main`.
        let status = origin_main(argc as usize, argv, envp);

        #[cfg(feature = "log")]
        log::trace!("`origin_main` returned `{:?}`", status);

        // Run functions registered with `at_exit`, and exit with
        // `origin_main`'s return value.
        exit(status)
    }
}

/// A program entry point similar to `_start`, but which is meant to be called
/// by something else in the program rather than the OS.
///
/// # Safety
///
/// `mem` must point to a stack with the contents that the OS would provide
/// on the initial stack.
#[cfg(feature = "external-start")]
pub unsafe fn start(mem: *mut usize) -> ! {
    entry(mem)
}

/// Compute `argc`, `argv`, and `envp`.
///
/// # Safety
///
/// `mem` must point to the stack as provided by the operating system.
#[cfg(feature = "origin-program")]
unsafe fn compute_args(mem: *mut usize) -> (i32, *mut *mut u8, *mut *mut u8) {
    use linux_raw_sys::ctypes::c_uint;

    let kernel_argc = *mem;
    let argc = kernel_argc as c_int;
    let argv = mem.add(1).cast::<*mut u8>();
    let envp = argv.add(argc as c_uint as usize + 1);

    // Do a few more precondition checks on `argc` and `argv`.
    debug_assert!(argc >= 0);
    debug_assert_eq!(kernel_argc, argc as _);
    debug_assert_eq!(*argv.add(argc as usize), core::ptr::null_mut());

    (argc, argv, envp)
}

/// Perform dynamic relocation (if enabled), and initialize `origin` and
/// `rustix` runtime state.
///
/// # Safety
///
/// `mem` must point to the stack as provided by the operating system. `envp`
/// must point to the incoming environment variables.
#[cfg(feature = "origin-program")]
#[allow(unused_variables)]
unsafe fn init_runtime(mem: *mut usize, envp: *mut *mut u8) {
    // Before doing anything else, perform dynamic relocations.
    #[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
    #[cfg(relocation_model = "pic")]
    crate::relocate::relocate(envp);

    // Explicitly initialize `rustix`. This is needed for things like
    // `page_size()` to work.
    #[cfg(feature = "param")]
    rustix::param::init(envp);

    // Read the program headers and extract the TLS info.
    #[cfg(feature = "origin-thread")]
    initialize_startup_thread_info();

    // Initialize the main thread.
    #[cfg(feature = "origin-thread")]
    initialize_main_thread(mem.cast());
}

/// Functions registered with [`at_exit`].
#[cfg(all(feature = "alloc", feature = "origin-program"))]
static DTORS: Mutex<Vec<Box<dyn FnOnce() + Send>>> = Mutex::new(Vec::new());

/// Register a function to be called when [`exit`] is called.
///
/// # Safety
///
/// This arranges for `func` to be called, and passed `obj`, when the program
/// exits.
#[cfg(feature = "alloc")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "alloc")))]
pub fn at_exit(func: Box<dyn FnOnce() + Send>) {
    #[cfg(feature = "origin-program")]
    {
        let mut funcs = DTORS.lock();
        funcs.push(func);
    }

    #[cfg(not(feature = "origin-program"))]
    {
        use core::ffi::c_void;

        extern "C" {
            // <https://refspecs.linuxbase.org/LSB_3.1.0/LSB-generic/LSB-generic/baselib---cxa-atexit.html>
            fn __cxa_atexit(
                func: unsafe extern "C" fn(*mut c_void),
                arg: *mut c_void,
                _dso: *mut c_void,
            ) -> c_int;
        }

        // The function to pass to `__cxa_atexit`.
        unsafe extern "C" fn at_exit_func(arg: *mut c_void) {
            Box::from_raw(arg as *mut Box<dyn FnOnce() + Send>)();
        }

        let at_exit_arg = Box::into_raw(Box::new(func)).cast::<c_void>();
        let r = unsafe { __cxa_atexit(at_exit_func, at_exit_arg, null_mut()) };
        assert_eq!(r, 0);
    }
}

/// Call all the functions registered with [`at_exit`] or with the
/// `.fini_array` section, and exit the program.
pub fn exit(status: c_int) -> ! {
    // Call functions registered with `at_thread_exit`.
    #[cfg(all(feature = "thread", feature = "origin-program"))]
    crate::thread::call_thread_dtors(crate::thread::current_thread());

    // Call all the registered functions, in reverse order. Leave `DTORS`
    // unlocked while making the call so that functions can add more functions
    // to the end of the list.
    #[cfg(all(feature = "alloc", feature = "origin-program"))]
    loop {
        let mut dtors = DTORS.lock();
        if let Some(dtor) = dtors.pop() {
            #[cfg(feature = "log")]
            log::trace!("Calling `at_exit`-registered function");

            dtor();
        } else {
            break;
        }
    }

    // Call the `.fini_array` functions, in reverse order. We only do this
    // in "origin-program" mode because if we're using libc, libc does this
    // in `exit`.
    #[cfg(all(feature = "init-fini-arrays", feature = "origin-program"))]
    unsafe {
        use core::arch::asm;
        use core::ffi::c_void;

        // The linker-generated symbols that mark the start and end of the
        // `.fini_array` section.
        extern "C" {
            static __fini_array_start: c_void;
            static __fini_array_end: c_void;
        }

        // Call the `.fini_array` functions.
        type FiniFn = extern "C" fn();
        let mut fini = &__fini_array_end as *const _ as *const FiniFn;
        let fini_start = &__fini_array_start as *const _ as *const FiniFn;
        // Prevent the optimizer from optimizing the `!=` comparison to true;
        // `fini` and `fini_start` may have the same address.
        asm!("# {}", inout(reg) fini, options(pure, nomem, nostack, preserves_flags));

        while fini != fini_start {
            fini = fini.sub(1);

            #[cfg(feature = "log")]
            log::trace!("Calling `.fini_array`-registered function `{:?}()`", *fini);

            (*fini)();
        }
    }

    #[cfg(feature = "origin-program")]
    {
        // Call `exit_immediately` to exit the program.
        exit_immediately(status)
    }

    #[cfg(not(feature = "origin-program"))]
    unsafe {
        // Call `libc` to run *its* dtors, and exit the program.
        libc::exit(status)
    }
}

/// Exit the program without calling functions registered with [`at_exit`] or
/// with the `.fini_array` section.
#[inline]
pub fn exit_immediately(status: c_int) -> ! {
    #[cfg(feature = "origin-program")]
    {
        #[cfg(feature = "log")]
        log::trace!("Program exiting with status `{:?}`", status);

        // Call `rustix` to exit the program.
        rustix::runtime::exit_group(status)
    }

    #[cfg(not(feature = "origin-program"))]
    unsafe {
        // Call `libc` to exit the program.
        libc::_exit(status)
    }
}

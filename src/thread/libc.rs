//! Thread startup and shutdown.
//!
//! Why does this api look like `thread::join(t)` instead of `t.join()`? Either
//! way could work, but free functions help emphasize that this API's
//! [`Thread`] differs from `std::thread::Thread`. It does not detach or free
//! its resources on drop, and does not guarantee validity. That gives users
//! more control when creating efficient higher-level abstractions like
//! pthreads or `std::thread::Thread`.

#[cfg(feature = "thread-at-exit")]
use alloc::boxed::Box;
use core::ffi::{c_int, c_void};
use core::mem::{size_of, transmute, zeroed};
use core::ptr::{NonNull, null_mut, with_exposed_provenance_mut, without_provenance_mut};
use core::slice;
use rustix::io;

pub use rustix::thread::Pid as ThreadId;

// Symbols defined in libc but not declared in the libc crate.
unsafe extern "C" {
    fn __cxa_thread_atexit_impl(
        func: unsafe extern "C" fn(*mut c_void),
        obj: *mut c_void,
        _dso_symbol: *mut c_void,
    ) -> libc::c_int;
    fn __errno_location() -> *mut c_int;
    fn __tls_get_addr(p: &[usize; 2]) -> *mut c_void;

    #[cfg(feature = "thread-at-exit")]
    static __dso_handle: *const c_void;
}

/// An opaque pointer to a thread.
///
/// This type does not detach or free resources on drop. It just leaks the
/// thread. To detach or join, call [`detach`] or [`join`] explicitly.
// In this library, we assume `libc::pthread` can be casted to and from a
// `usize` and directly compared for equality (without using `pthread_equal`).
// POSIX says that `pthread_t` can be a struct type; if any platform ever
// does that, the code below won't compile, and we'll have to figure out what
// to do about it.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Thread(libc::pthread_t);

impl Thread {
    /// Convert to `Self` from a raw pointer that was returned from
    /// `Thread::to_raw`.
    #[inline]
    pub fn from_raw(raw: *mut c_void) -> Self {
        Self(raw.expose_provenance() as libc::pthread_t)
    }

    /// Convert to `Self` from a raw non-null pointer.
    ///
    /// # Safety
    ///
    /// `raw` must be a valid non-null thread pointer.
    #[inline]
    pub unsafe fn from_raw_unchecked(raw: *mut c_void) -> Self {
        Self::from_raw(raw)
    }

    /// Convert to `Self` from a raw non-null pointer that was returned from
    /// `Thread::to_raw_non_null`.
    #[inline]
    pub fn from_raw_non_null(raw: NonNull<c_void>) -> Self {
        Self::from_raw(raw.as_ptr())
    }

    /// Convert to a raw pointer from a `Self`.
    ///
    /// This value is guaranteed to uniquely identify a thread, while it is
    /// running. After a thread has exited, this value may be reused by new
    /// threads.
    #[inline]
    pub fn to_raw(self) -> *mut c_void {
        with_exposed_provenance_mut(self.0 as usize)
    }

    /// Convert to a raw non-null pointer from a `Self`.
    ///
    /// This value is guaranteed to uniquely identify a thread, while it is
    /// running. After a thread has exited, this value may be reused by new
    /// threads.
    #[inline]
    pub fn to_raw_non_null(self) -> NonNull<c_void> {
        NonNull::new(self.to_raw()).unwrap()
    }
}

/// Creates a new thread.
///
/// `fn_(args)` is called on the new thread, except that the argument values
/// copied to memory that can be exclusively referenced by the thread.
///
/// # Safety
///
/// The values of `args` must be valid to send to the new thread, `fn_(args)`
/// on the new thread must have defined behavior, and the return value must be
/// valid to send to other threads.
pub unsafe fn create(
    fn_: unsafe fn(&mut [Option<NonNull<c_void>>]) -> Option<NonNull<c_void>>,
    args: &[Option<NonNull<c_void>>],
    stack_size: usize,
    guard_size: usize,
) -> io::Result<Thread> {
    // This should really be an `unsafe` function, but `libc::pthread_create`
    // doesn't have `unsafe` in its signature.
    extern "C" fn start(thread_arg_ptr: *mut c_void) -> *mut c_void {
        unsafe {
            // Unpack the thread arguments.
            let thread_arg_ptr = thread_arg_ptr.cast::<Option<NonNull<c_void>>>();
            let num_args = match thread_arg_ptr.read() {
                Some(ptr) => ptr.as_ptr().addr(),
                None => 0,
            };
            let thread_args = slice::from_raw_parts_mut(thread_arg_ptr, num_args + 2);
            let fn_: unsafe fn(&mut [Option<NonNull<c_void>>]) -> Option<NonNull<c_void>> =
                transmute(thread_args[1]);
            let args = &mut thread_args[2..];

            // Call the user's function.
            let return_value = fn_(args);

            // Free the thread argument memory.
            libc::free(thread_arg_ptr.cast::<c_void>());

            // Return the return value.
            match return_value {
                Some(return_value) => return_value.as_ptr(),
                None => null_mut(),
            }
        }
    }

    unsafe {
        let mut attr: libc::pthread_attr_t = zeroed();
        match libc::pthread_attr_init(&mut attr) {
            0 => (),
            err => return Err(io::Errno::from_raw_os_error(err)),
        }
        match libc::pthread_attr_setstacksize(&mut attr, stack_size) {
            0 => (),
            err => return Err(io::Errno::from_raw_os_error(err)),
        }
        match libc::pthread_attr_setguardsize(&mut attr, guard_size) {
            0 => (),
            err => return Err(io::Errno::from_raw_os_error(err)),
        }

        let mut new_thread = zeroed();

        // Pack up the thread arguments.
        let thread_arg_ptr = libc::malloc((args.len() + 2) * size_of::<*mut c_void>());
        if thread_arg_ptr.is_null() {
            return Err(io::Errno::NOMEM);
        }
        let thread_args = slice::from_raw_parts_mut(
            thread_arg_ptr.cast::<Option<NonNull<c_void>>>(),
            args.len() + 2,
        );
        thread_args[0] = NonNull::new(without_provenance_mut(args.len()));
        thread_args[1] = NonNull::new(fn_ as _);
        thread_args[2..].copy_from_slice(args);

        // Call libc to create the thread.
        match libc::pthread_create(&mut new_thread, &attr, start, thread_arg_ptr) {
            0 => (),
            err => return Err(io::Errno::from_raw_os_error(err)),
        }

        Ok(Thread(new_thread))
    }
}

/// Marks a thread as “detached”.
///
/// Detached threads free their own resources automatically when they
/// exit, rather than when they are joined.
///
/// # Safety
///
/// `thread` must point to a valid thread record that has not yet been detached
/// and will not be joined.
#[inline]
pub unsafe fn detach(thread: Thread) {
    unsafe {
        let thread = thread.0;

        assert_eq!(libc::pthread_detach(thread), 0);
    }
}

/// Waits for a thread to finish.
///
/// The return value is the value returned from the call to the `fn_` passed to
/// `create_thread`.
///
/// # Safety
///
/// `thread` must point to a valid thread record that has not already been
/// detached or joined.
pub unsafe fn join(thread: Thread) -> Option<NonNull<c_void>> {
    unsafe {
        let thread = thread.0;

        let mut return_value: *mut c_void = null_mut();
        assert_eq!(libc::pthread_join(thread, &mut return_value), 0);

        NonNull::new(return_value)
    }
}

/// Registers a function to call when the current thread exits.
#[cfg(feature = "thread-at-exit")]
pub fn at_exit(func: Box<dyn FnOnce()>) {
    unsafe extern "C" fn call(arg: *mut c_void) {
        unsafe {
            let arg = arg.cast::<Box<dyn FnOnce()>>();
            let arg = Box::from_raw(arg);
            arg()
        }
    }

    unsafe {
        let arg = Box::into_raw(Box::new(func));

        assert_eq!(__cxa_thread_atexit_impl(call, arg.cast(), dso_handle()), 0);
    }
}

/// Return a raw pointer to the data associated with the current thread.
#[inline]
#[must_use]
pub fn current() -> Thread {
    unsafe { Thread(libc::pthread_self()) }
}

/// Return the current thread id.
///
/// This is the same as [`rustix::thread::gettid`], but loads the value from a
/// field in the runtime rather than making a system call.
#[inline]
#[must_use]
pub fn current_id() -> ThreadId {
    unsafe { ThreadId::from_raw_unchecked(libc::gettid()) }
}

/// Set the current thread id, after a `fork`.
///
/// The only valid use for this is in the implementation of libc-like `fork`
/// wrappers such as the one in c-scape. `posix_spawn`-like uses of `fork`
/// don't need to do this because they shouldn't do anything that cares about
/// the thread id before doing their `execve`.
///
/// # Safety
///
/// This must only be called immediately after a `fork` before any other
/// threads are created. `tid` must be the same value as what [`gettid`] would
/// return.
#[doc(hidden)]
#[inline]
pub unsafe fn set_current_id_after_a_fork(tid: ThreadId) {
    // Nothing to do here; libc does the update automatically.
    let _ = tid;
}

/// Return the address of the thread-local `errno` state.
///
/// This is equivalent to `__errno_location()` in glibc and musl.
#[cfg(feature = "unstable-errno")]
#[inline]
pub fn errno_location() -> *mut i32 {
    unsafe { __errno_location() }
}

/// Return the TLS address for the given `module` and `offset` for the current
/// thread.
#[inline]
#[must_use]
pub fn current_tls_addr(module: usize, offset: usize) -> *mut c_void {
    unsafe { __tls_get_addr(&[module, offset]) }
}

/// Return the current thread's stack address (lowest address), size, and guard
/// size.
///
/// # Safety
///
/// `thread` must point to a valid thread record.
#[inline]
#[must_use]
pub unsafe fn stack(thread: Thread) -> (*mut c_void, usize, usize) {
    unsafe {
        let thread = thread.0;

        let mut attr: libc::pthread_attr_t = zeroed();
        assert_eq!(libc::pthread_getattr_np(thread, &mut attr), 0);

        let mut stack_size = 0;
        let mut stack_addr = null_mut();
        assert_eq!(
            libc::pthread_attr_getstack(&attr, &mut stack_addr, &mut stack_size),
            0
        );

        let mut guard_size = 0;
        assert_eq!(libc::pthread_attr_getguardsize(&attr, &mut guard_size), 0);

        (stack_addr, stack_size, guard_size)
    }
}

/// Return the default stack size for new threads.
#[inline]
#[must_use]
pub fn default_stack_size() -> usize {
    unsafe {
        let mut attr: libc::pthread_attr_t = zeroed();
        assert_eq!(libc::pthread_getattr_np(libc::pthread_self(), &mut attr), 0);

        let mut stack_size = 0;
        assert_eq!(libc::pthread_attr_getstacksize(&attr, &mut stack_size), 0);

        stack_size
    }
}

/// Return the default guard size for new threads.
#[inline]
#[must_use]
pub fn default_guard_size() -> usize {
    unsafe {
        let mut attr: libc::pthread_attr_t = zeroed();
        assert_eq!(libc::pthread_getattr_np(libc::pthread_self(), &mut attr), 0);

        let mut guard_size = 0;
        assert_eq!(libc::pthread_attr_getguardsize(&attr, &mut guard_size), 0);

        guard_size
    }
}

/// Yield the current thread, encouraging other threads to run.
#[inline]
pub fn yield_current() {
    let _ = unsafe { libc::sched_yield() };
}

/// Return the address of `__dso_handle`, appropriately casted.
#[cfg(feature = "thread-at-exit")]
unsafe fn dso_handle() -> *mut c_void {
    unsafe {
        let dso_handle: *const *const c_void = &__dso_handle;
        dso_handle.cast::<c_void>().cast_mut()
    }
}

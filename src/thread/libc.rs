//! Thread startup and shutdown.

use alloc::boxed::Box;
use core::ffi::c_void;
use core::mem::{size_of, transmute, zeroed};
use core::ptr::{from_exposed_addr_mut, invalid_mut, null_mut, NonNull};
use core::slice;
use rustix::io;

pub use rustix::thread::Pid as ThreadId;

// Symbols defined in libc but not declared in the libc crate.
extern "C" {
    fn __cxa_thread_atexit_impl(
        func: unsafe extern "C" fn(*mut c_void),
        obj: *mut c_void,
        _dso_symbol: *mut c_void,
    ) -> libc::c_int;
    fn __tls_get_addr(p: &[usize; 2]) -> *mut c_void;

    static __dso_handle: *const c_void;
}

/// An opaque pointer to a thread.
///
/// This type does not detach on drop. It just leaks the thread. To detach or
/// join, call `detach_thread` or `join_thread` explicitly.
#[derive(Copy, Clone)]
pub struct Thread(libc::pthread_t);

impl Thread {
    /// Convert to `Self` from a raw pointer.
    #[inline]
    pub fn from_raw(raw: *mut c_void) -> Self {
        Self(raw.expose_addr() as libc::pthread_t)
    }

    /// Convert to `Self` from a raw non-null pointer.
    #[inline]
    pub fn from_raw_non_null(raw: NonNull<c_void>) -> Self {
        Self::from_raw(raw.as_ptr())
    }

    /// Convert to a raw pointer from a `Self`.
    #[inline]
    pub fn to_raw(self) -> *mut c_void {
        from_exposed_addr_mut(self.0 as usize)
    }

    /// Convert to a raw non-null pointer from a `Self`.
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
/// `fn_(arg)` on the new thread must have defined behavior, and the return
/// value must be valid to use on the eventual joining thread.
pub unsafe fn create_thread(
    fn_: unsafe fn(&mut [*mut c_void]) -> Option<NonNull<c_void>>,
    args: &[Option<NonNull<c_void>>],
    stack_size: usize,
    guard_size: usize,
) -> io::Result<Thread> {
    extern "C" fn start(thread_arg_ptr: *mut c_void) -> *mut c_void {
        unsafe {
            // Unpack the thread arguments.
            let num_args = thread_arg_ptr.cast::<*mut c_void>().read().addr();
            let thread_args =
                slice::from_raw_parts_mut(thread_arg_ptr.cast::<*mut c_void>(), num_args + 2);
            let fn_: unsafe fn(&mut [*mut c_void]) -> Option<NonNull<c_void>> =
                transmute(thread_args[1]);
            let args = &mut thread_args[2..];

            // Call the user's function.
            let return_value = fn_(args);

            // Free the thread argument memory.
            libc::free(thread_arg_ptr);

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
        thread_args[0] = NonNull::new(invalid_mut(args.len()));
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

/// Registers a function to call when the current thread exits.
pub fn at_thread_exit(func: Box<dyn FnOnce()>) {
    extern "C" fn call(arg: *mut c_void) {
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
pub fn current_thread() -> Thread {
    unsafe { Thread(libc::pthread_self()) }
}

/// Return the current thread id.
///
/// This is the same as [`rustix::thread::gettid`], but loads the value from a
/// field in the runtime rather than making a system call.
#[inline]
#[must_use]
pub fn current_thread_id() -> ThreadId {
    // Actually, in the pthread implementation here we do just make a system
    // call, because we don't have access to the pthread internals.
    rustix::thread::gettid()
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
#[cfg(feature = "set_thread_id")]
#[doc(hidden)]
#[inline]
pub unsafe fn set_current_thread_id_after_a_fork(tid: ThreadId) {
    // Nothing to do here; libc does the update automatically.
    let _ = tid;
}

/// Return the TLS address for the given `offset` for the current thread.
#[inline]
#[must_use]
pub fn current_thread_tls_addr(offset: usize) -> *mut c_void {
    let p = [1, offset];
    unsafe { __tls_get_addr(&p) }
}

/// Return the current thread's stack address (lowest address), size, and guard
/// size.
///
/// # Safety
///
/// `thread` must point to a valid thread record.
#[inline]
pub unsafe fn thread_stack(thread: Thread) -> (*mut c_void, usize, usize) {
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
pub unsafe fn detach_thread(thread: Thread) {
    let thread = thread.0;

    assert_eq!(libc::pthread_detach(thread), 0);
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
pub unsafe fn join_thread(thread: Thread) -> Option<NonNull<c_void>> {
    let thread = thread.0;

    let mut return_value: *mut c_void = null_mut();
    assert_eq!(libc::pthread_join(thread, &mut return_value), 0);

    NonNull::new(return_value)
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
pub fn yield_current_thread() {
    let _ = unsafe { libc::sched_yield() };
}

/// Return the address of `__dso_handle`, appropriately casted.
unsafe fn dso_handle() -> *mut c_void {
    let dso_handle: *const *const c_void = &__dso_handle;
    dso_handle.cast::<c_void>().cast_mut()
}

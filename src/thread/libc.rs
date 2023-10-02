//! Thread startup and shutdown.

use alloc::boxed::Box;
use core::any::Any;
use core::ffi::c_void;
use core::ptr::{from_exposed_addr_mut, null_mut, NonNull};
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

    // TODO: PR for these has been submitted to upstream:
    // <https://github.com/rust-lang/libc/pull/3330>
    fn pthread_attr_getstacksize(
        attr: *const libc::pthread_attr_t,
        stacksize: &mut libc::size_t,
    ) -> libc::c_int;
    fn pthread_attr_setguardsize(
        attr: *mut libc::pthread_attr_t,
        guardsize: libc::size_t,
    ) -> libc::c_int;

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
/// `fn_` is called on the new thread.
pub fn create_thread(
    fn_: Box<dyn FnOnce() -> Option<Box<dyn Any>>>,
    stack_size: usize,
    guard_size: usize,
) -> io::Result<Thread> {
    extern "C" fn start(arg: *mut c_void) -> *mut c_void {
        unsafe {
            let arg = arg.cast::<Box<dyn FnOnce() -> Option<Box<dyn Any>>>>();
            let arg = Box::from_raw(arg);
            match arg() {
                None => null_mut(),
                Some(ret) => Box::into_raw(ret).cast(),
            }
        }
    }

    unsafe {
        let mut attr: libc::pthread_attr_t = core::mem::zeroed();
        match libc::pthread_attr_init(&mut attr) {
            0 => (),
            err => return Err(io::Errno::from_raw_os_error(err)),
        }
        match libc::pthread_attr_setstacksize(&mut attr, stack_size) {
            0 => (),
            err => return Err(io::Errno::from_raw_os_error(err)),
        }
        match pthread_attr_setguardsize(&mut attr, guard_size) {
            0 => (),
            err => return Err(io::Errno::from_raw_os_error(err)),
        }
        let mut new_thread = core::mem::zeroed();
        let arg = Box::into_raw(Box::new(fn_));
        match libc::pthread_create(&mut new_thread, &attr, start, arg.cast()) {
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

    let mut attr: libc::pthread_attr_t = core::mem::zeroed();
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

/// Marks a thread as "detached".
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
/// # Safety
///
/// `thread` must point to a valid thread record that has not already been
/// detached or joined.
pub unsafe fn join_thread(thread: Thread) {
    let thread = thread.0;

    let mut retval: *mut c_void = null_mut();
    assert_eq!(libc::pthread_join(thread, &mut retval), 0);
}

/// Return the default stack size for new threads.
#[inline]
#[must_use]
pub fn default_stack_size() -> usize {
    unsafe {
        let mut attr: libc::pthread_attr_t = core::mem::zeroed();
        assert_eq!(libc::pthread_getattr_np(libc::pthread_self(), &mut attr), 0);

        let mut stack_size = 0;
        assert_eq!(pthread_attr_getstacksize(&attr, &mut stack_size), 0);

        stack_size
    }
}

/// Return the default guard size for new threads.
#[inline]
#[must_use]
pub fn default_guard_size() -> usize {
    unsafe {
        let mut attr: libc::pthread_attr_t = core::mem::zeroed();
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
    dso_handle as *mut c_void
}

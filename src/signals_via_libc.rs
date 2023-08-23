use core::mem::MaybeUninit;
use core::ptr::null;
use rustix::io;
use rustix::process::Signal;

/// A signal action record for use with [`sigaction`].
pub type Sigaction = libc::sigaction;

/// Register a signal handler.
///
/// # Safety
///
/// yolo
pub unsafe fn sigaction(sig: Signal, action: Option<Sigaction>) -> io::Result<Sigaction> {
    let action: *const Sigaction = match action {
        Some(action) => &action,
        None => null(),
    };
    let mut old = MaybeUninit::<Sigaction>::uninit();

    if libc::sigaction(sig as libc::c_int, action, old.as_mut_ptr()) == 0 {
        Ok(old.assume_init())
    } else {
        // FIXME: this needs https://github.com/lambda-fairy/rust-errno/pull/77
        //Err(rustix::io::Errno::from_raw_os_error(errno::errno().0))
        Err(rustix::io::Errno::from_raw_os_error(libc::EINVAL))
    }
}

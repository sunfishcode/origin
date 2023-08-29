//! Signal handlers.

use core::mem::MaybeUninit;
use core::ptr::null;
use rustix::io;

/// A signal action record for use with [`sigaction`].
pub type Sigaction = libc::sigaction;

/// A signal identifier for use with [`sigaction`].
pub use rustix::runtime::Signal;

/// A signal handler function for use with [`Sigaction`].
pub use libc::sighandler_t as Sighandler;

/// A signal information record for use with [`Sigaction`].
// TODO: Convert the fields of this to friendlier APIs.
pub use linux_raw_sys::general::siginfo_t as Siginfo;

/// A flags type for use with [`Sigaction`].
pub use linux_raw_sys::ctypes::c_int as Sigflags;

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

/// Return a special "ignore" signal handler for ignoring signals.
#[doc(alias = "SIG_IGN")]
pub fn sig_ign() -> Sighandler {
    libc::SIG_IGN
}

/// `SIG_DFL`
pub use libc::SIG_DFL as SigDfl;

// TODO: Convert these to a `bitflags`?

/// `SA_RESTART`
pub const SA_RESTART: Sigflags = libc::SA_RESTART;
/// `SA_ONSTACK`
pub const SA_ONSTACK: Sigflags = libc::SA_ONSTACK;
/// `SA_SIGINFO`
pub const SA_SIGINFO: Sigflags = libc::SA_SIGINFO;

/// `SIGSTKSZ`
pub const SIGSTKSZ: usize = libc::SIGSTKSZ as usize;
/// `SS_DISABLE`
pub const SS_DISABLE: i32 = libc::SS_DISABLE;

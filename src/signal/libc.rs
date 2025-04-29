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
pub use libc::siginfo_t as Siginfo;

bitflags::bitflags! {
    /// A flags type for use with [`Sigaction`].
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
    pub struct SigactionFlags: libc::c_int {
        /// `SA_NOCLDSTOP`
        const NOCLDSTOP = libc::SA_NOCLDSTOP as _;

        /// `SA_NOCLDWAIT` (since Linux 2.6)
        const NOCLDWAIT = libc::SA_NOCLDWAIT as _;

        /// `SA_NODEFER`
        const NODEFER = libc::SA_NODEFER as _;

        /// `SA_ONSTACK`
        const ONSTACK = libc::SA_ONSTACK as _;

        /// `SA_RESETHAND`
        const RESETHAND = libc::SA_RESETHAND as _;

        /// `SA_RESTART`
        const RESTART = libc::SA_RESTART as _;

        /// `SA_SIGINFO` (since Linux 2.2)
        const SIGINFO = libc::SA_SIGINFO as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// Register a signal handler.
///
/// # Safety
///
/// yolo. At least this function handles `sa_restorer` automatically though.
pub unsafe fn sigaction(sig: Signal, action: Option<Sigaction>) -> io::Result<Sigaction> {
    unsafe {
        let action: *const Sigaction = match action {
            Some(action) => &action,
            None => null(),
        };
        let mut old = MaybeUninit::<Sigaction>::uninit();

        if libc::sigaction(sig.as_raw(), action, old.as_mut_ptr()) == 0 {
            Ok(old.assume_init())
        } else {
            Err(rustix::io::Errno::from_raw_os_error(errno::errno().0))
        }
    }
}

/// Return a special “ignore” signal handler for ignoring signals.
///
/// If you're looking for `sig_dfl`; use [`SigDfl`].
#[doc(alias = "SIG_IGN")]
#[must_use]
pub const fn sig_ign() -> Sighandler {
    libc::SIG_IGN
}

/// A special “default” signal handler representing the default behavior
/// for handling a signal.
///
/// If you're looking for `SigIgn`; use [`sig_ign`].
pub use libc::SIG_DFL;

/// `SIGSTKSZ`
pub const SIGSTKSZ: usize = libc::SIGSTKSZ;
/// `SS_DISABLE`
pub const SS_DISABLE: i32 = libc::SS_DISABLE;

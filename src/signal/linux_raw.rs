//! Signal handlers.

use rustix::io;
#[cfg(not(target_arch = "riscv64"))]
use {crate::arch, linux_raw_sys::ctypes::c_ulong, linux_raw_sys::general::SA_RESTORER};

/// A signal action record for use with [`sigaction`].
pub use rustix::runtime::Sigaction;

/// A signal identifier for use with [`sigaction`].
pub use rustix::runtime::Signal;

/// A signal handler function for use with [`Sigaction`].
pub use linux_raw_sys::general::__kernel_sighandler_t as Sighandler;

/// A signal information record for use with [`Sigaction`].
// TODO: Convert the fields of this to friendlier APIs.
pub use linux_raw_sys::general::siginfo_t as Siginfo;

/// A flags type for use with [`Sigaction`].
pub use linux_raw_sys::ctypes::c_ulong as Sigflags;

/// Register a signal handler.
///
/// # Safety
///
/// yolo. At least this function handles `sa_restorer` automatically though.
pub unsafe fn sigaction(sig: Signal, action: Option<Sigaction>) -> io::Result<Sigaction> {
    #[allow(unused_mut)]
    let mut action = action;

    #[cfg(not(target_arch = "riscv64"))]
    if let Some(action) = &mut action {
        action.sa_flags |= SA_RESTORER as c_ulong;

        if (action.sa_flags & SA_SIGINFO as c_ulong) == SA_SIGINFO as c_ulong {
            action.sa_restorer = Some(arch::return_from_signal_handler);
        } else {
            action.sa_restorer = Some(arch::return_from_signal_handler_noinfo);
        }
    }

    rustix::runtime::sigaction(sig, action)
}

/// Return a special "ignore" signal handler for ignoring signals.
#[doc(alias = "SIG_IGN")]
pub fn sig_ign() -> Sighandler {
    linux_raw_sys::signal_macros::sig_ign()
}

/// `SIG_DFL`
pub use linux_raw_sys::signal_macros::SIG_DFL as SigDfl;

// TODO: Convert these to a `bitflags`.

/// `SA_RESTART`
pub const SA_RESTART: Sigflags = linux_raw_sys::general::SA_RESTART as _;
/// `SA_ONSTACK`
pub const SA_ONSTACK: Sigflags = linux_raw_sys::general::SA_ONSTACK as _;
/// `SA_SIGINFO`
pub const SA_SIGINFO: Sigflags = linux_raw_sys::general::SA_SIGINFO as _;

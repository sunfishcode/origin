use rustix::io;
#[cfg(not(target_arch = "riscv64"))]
use {
    crate::arch,
    linux_raw_sys::ctypes::c_ulong,
    linux_raw_sys::general::{SA_RESTORER, SA_SIGINFO},
};

/// A signal action record for use with [`sigaction`].
pub use rustix::runtime::Sigaction;

/// A signal identifiier for use with [`sigaction`].
pub use rustix::process::Signal;

/// A signal handler function for use with [`Sigaction`].
pub use linux_raw_sys::general::__kernel_sighandler_t as Sighandler;

/// A flags type for use with [`Sigaction`].
pub use linux_raw_sys::ctypes::c_ulong as Sigflags;

/// Register a signal handler.
///
/// # Safety
///
/// yolo
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

/// `SA_RESTART`
pub const SA_RESTART: Sigflags = linux_raw_sys::general::SA_RESTART as _;

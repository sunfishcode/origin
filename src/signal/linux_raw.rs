//! Signal handlers.

#[cfg(not(target_arch = "riscv64"))]
use crate::arch;
use rustix::io;

pub use rustix::runtime::{
    KernelSigaction as Sigaction, KernelSigactionFlags as SigactionFlags, Siginfo, Signal,
};

/// A handler function type for use with [`Sigaction`].
pub type Sighandler = rustix::runtime::KernelSighandler;

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
        action.sa_flags |= SigactionFlags::RESTORER;

        if action.sa_flags.contains(SigactionFlags::SIGINFO) {
            action.sa_restorer = Some(arch::return_from_signal_handler);
        } else {
            action.sa_restorer = Some(arch::return_from_signal_handler_noinfo);
        }
    }

    rustix::runtime::kernel_sigaction(sig, action)
}

/// Return a special “ignore” signal handler for ignoring signals.
///
/// If you're looking for `sig_dfl`; use [`SigDfl`].
#[doc(alias = "SIG_IGN")]
#[must_use]
pub const fn sig_ign() -> Sighandler {
    rustix::runtime::kernel_sig_ign()
}

/// A special “default” signal handler representing the default behavior
/// for handling a signal.
///
/// If you're looking for `SigIgn`; use [`sig_ign`].
pub use rustix::runtime::KERNEL_SIG_DFL as SIG_DFL;

/// `SIGSTKSZ`
pub const SIGSTKSZ: usize = linux_raw_sys::general::SIGSTKSZ as usize;
/// `SS_DISABLE`
pub const SS_DISABLE: i32 = linux_raw_sys::general::SS_DISABLE as i32;

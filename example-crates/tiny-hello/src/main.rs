//! Going for minimal size!

#![no_std]
#![no_main]

extern crate origin;

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    let message = b"Hello, world!\n";
    let mut remaining = &message[..];
    while !remaining.is_empty() {
        match rustix::io::write(rustix::stdio::stdout(), message) {
            Ok(n) => remaining = &remaining[n as usize..],
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => origin::program::trap(),
        }
    }
    origin::program::immediate_exit(0);
}

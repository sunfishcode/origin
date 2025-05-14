/// Initialize logging, if enabled.
///
/// c-scape initializes its environment-variable state at
/// `.init_array.00098`, so using `.00099` means we run after that, so
/// initializing loggers that depend on eg. `RUST_LOG` work.
#[unsafe(link_section = ".init_array.00099")]
#[used]
static INIT_ARRAY: unsafe extern "C" fn() = {
    unsafe extern "C" fn function() {
        init()
    }
    function
};

fn init() {
    // Initialize the chosen logger.
    #[cfg(feature = "atomic-dbg-logger")]
    atomic_dbg::log::init();
    #[cfg(feature = "env_logger")]
    env_logger::init();

    // Log the first message, announcing that it is us that started the
    // process. We started the program earlier than this, but we couldn't
    // initialize the logger until we initialized the main thread.
    #[cfg(feature = "take-charge")]
    log::trace!(target: "origin::program", "Program started");

    // Log the main thread id. Similarly, we initialized the main thread
    // earlier than this, but we couldn't log until now.
    #[cfg(all(feature = "take-charge", feature = "thread"))]
    log::trace!(
        target: "origin::thread",
        "Main Thread[{:?}] initialized",
        crate::thread::current_id().as_raw_nonzero()
    );
}

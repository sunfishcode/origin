/// Initialize logging, if enabled.
#[link_section = ".init_array.00099"]
#[used]
static INIT_ARRAY: unsafe extern "C" fn() = {
    unsafe extern "C" fn function() {
        // Initialize the chosen logger.
        #[cfg(feature = "atomic-dbg")]
        atomic_dbg::log::init();
        #[cfg(feature = "env_logger")]
        env_logger::init();

        // Log the first message, announcing that it is us that started the
        // process. We started the program earlier than this, but we couldn't
        // initialize the logger until we initialized the main thread.
        log::trace!(target: "origin::program", "Program started");

        // Log the main thread id. Similarly, we initialized the main thread
        // earlier than this, but we couldn't log until now.
        #[cfg(feature = "origin-thread")]
        log::trace!(
            target: "origin::thread",
            "Main Thread[{:?}] initialized",
            thread::current_thread_id().as_raw_nonzero()
        );
    }
    function
};

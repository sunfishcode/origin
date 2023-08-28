use origin::thread::*;
use origin::program::*;

fn main() {
    eprintln!("Hello from main thread");

    at_exit(Box::new(|| eprintln!("Hello from an at_exit handler")));
    at_thread_exit(Box::new(|| {
        eprintln!("Hello from a main-thread at_thread_exit handler")
    }));

    let thread = create_thread(
        Box::new(|| {
            eprintln!("Hello from child thread");
            at_thread_exit(Box::new(|| {
                eprintln!("Hello from child thread's at_thread_exit handler")
            }));
            None
        }),
        2 * 1024 * 1024,
        default_guard_size(),
    )
    .unwrap();

    unsafe {
        join_thread(thread);
    }

    eprintln!("Goodbye from main");
    exit(0);
}

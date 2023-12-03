use origin::{program, thread};

fn main() {
    eprintln!("Hello from main thread");

    program::at_exit(Box::new(|| {
        eprintln!("Hello from an `program::at_exit` handler")
    }));
    thread::at_exit(Box::new(|| {
        eprintln!("Hello from a main-thread `thread::at_exit` handler")
    }));

    let thread = unsafe {
        thread::create(
            |_args| {
                eprintln!("Hello from child thread");
                thread::at_exit(Box::new(|| {
                    eprintln!("Hello from child thread's `thread::at_exit` handler")
                }));
                None
            },
            &[],
            thread::default_stack_size(),
            thread::default_guard_size(),
        )
        .unwrap()
    };

    unsafe {
        thread::join(thread);
    }

    eprintln!("Goodbye from main");
    program::exit(0);
}

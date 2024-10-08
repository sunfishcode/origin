//! Test `thread::id` and `thread::current_id`.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::ffi::c_void;
use core::ptr::{null_mut, NonNull};
use origin::{program, thread};
use rustix_futex_sync::{Condvar, Mutex};

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[derive(Eq, PartialEq, Debug)]
enum Id {
    Parent(thread::ThreadId),
    Child(thread::ThreadId),
}

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    assert_eq!(thread::current_id(), thread::id(thread::current()).unwrap());
    program::at_exit(Box::new(|| {
        assert_eq!(thread::current_id(), thread::id(thread::current()).unwrap())
    }));
    thread::at_exit(Box::new(|| {
        assert_eq!(thread::current_id(), thread::id(thread::current()).unwrap());
    }));

    let cond = Arc::new((Mutex::new(None), Condvar::new()));
    let cond_clone = NonNull::new(Box::into_raw(Box::new(Arc::clone(&cond))).cast());

    let thread = thread::create(
        move |args| {
            assert_eq!(thread::current_id(), thread::id(thread::current()).unwrap());
            thread::at_exit(Box::new(|| {
                assert_eq!(thread::current_id(), thread::id(thread::current()).unwrap());
            }));

            let unpack = |x: Option<NonNull<c_void>>| match x {
                Some(p) => p.as_ptr().cast::<Arc<(Mutex<Option<Id>>, Condvar)>>(),
                None => null_mut(),
            };
            let cond = Box::from_raw(unpack(args[0]));

            // Wait for the parent to tell the child its id, and check it.
            let (lock, cvar) = &**cond;
            {
                let mut id = lock.lock();
                while (*id).is_none() {
                    id = cvar.wait(id);
                }
                assert_ne!(Id::Parent(thread::current_id()), *id.as_ref().unwrap());
            }

            // Tell the parent the child's id.
            {
                let mut id = lock.lock();
                *id = Some(Id::Child(thread::current_id()));
                cvar.notify_one();
            }

            assert_eq!(thread::current_id(), thread::id(thread::current()).unwrap());
            None
        },
        &[cond_clone],
        thread::default_stack_size(),
        thread::default_guard_size(),
    )
    .unwrap();

    // While the child is still running, examine its id.
    let child_thread_id_from_main = thread::id(thread).unwrap();

    // Tell the child the main thread id.
    let (lock, cvar) = &*cond;
    {
        let mut id = lock.lock();
        *id = Some(Id::Parent(thread::current_id()));
        cvar.notify_one();
    }

    // Wait for the child to tell the main thread its id, and check it.
    {
        let mut id = lock.lock();
        loop {
            if let Some(Id::Child(id)) = *id {
                assert_eq!(child_thread_id_from_main, id);
                break;
            }
            id = cvar.wait(id);
        }
    }

    // At some point the child thread should exit, at which point `thread::id`
    // will return `None`.
    while thread::id(thread).is_some() {
        let _ = rustix::thread::nanosleep(&rustix::thread::Timespec {
            tv_sec: 0,
            tv_nsec: 100_000_000,
        });
    }

    thread::join(thread);

    assert_eq!(thread::current_id(), thread::id(thread::current()).unwrap());
    program::exit(201);
}

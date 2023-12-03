//! Test `thread_id` and `current_thread_id`.

#![no_std]
#![no_main]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(core_intrinsics)]

extern crate alloc;
extern crate compiler_builtins;

use alloc::boxed::Box;
use alloc::sync::Arc;
use atomic_dbg::dbg;
use core::ffi::c_void;
use core::ptr::{null_mut, NonNull};
use origin::program::*;
use origin::thread::*;
use rustix_futex_sync::{Condvar, Mutex};

#[panic_handler]
fn panic(panic: &core::panic::PanicInfo<'_>) -> ! {
    dbg!(panic);
    core::intrinsics::abort();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[global_allocator]
static GLOBAL_ALLOCATOR: rustix_dlmalloc::GlobalDlmalloc = rustix_dlmalloc::GlobalDlmalloc;

#[derive(Eq, PartialEq, Debug)]
enum Id {
    Parent(ThreadId),
    Child(ThreadId),
}

#[no_mangle]
unsafe fn origin_main(_argc: usize, _argv: *mut *mut u8, _envp: *mut *mut u8) -> i32 {
    assert_eq!(current_thread_id(), thread_id(current_thread()).unwrap());
    at_exit(Box::new(|| {
        assert_eq!(current_thread_id(), thread_id(current_thread()).unwrap())
    }));
    at_thread_exit(Box::new(|| {
        assert_eq!(current_thread_id(), thread_id(current_thread()).unwrap());
    }));

    let cond = Arc::new((Mutex::new(None), Condvar::new()));
    let cond_clone = NonNull::new(Box::into_raw(Box::new(Arc::clone(&cond))).cast());

    let thread = create_thread(
        move |args| {
            assert_eq!(current_thread_id(), thread_id(current_thread()).unwrap());
            at_thread_exit(Box::new(|| {
                assert_eq!(current_thread_id(), thread_id(current_thread()).unwrap());
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
                assert_ne!(Id::Parent(current_thread_id()), *id.as_ref().unwrap());
            }

            // Tell the parent the child's id.
            {
                let mut id = lock.lock();
                *id = Some(Id::Child(current_thread_id()));
                cvar.notify_one();
            }

            assert_eq!(current_thread_id(), thread_id(current_thread()).unwrap());
            None
        },
        &[cond_clone],
        default_stack_size(),
        default_guard_size(),
    )
    .unwrap();

    // While the child is still running, examine its id.
    let child_thread_id_from_main = thread_id(thread).unwrap();

    // Tell the child the main thread id.
    let (lock, cvar) = &*cond;
    {
        let mut id = lock.lock();
        *id = Some(Id::Parent(current_thread_id()));
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

    // At some point the child thread should exit, at which point `thread_id`
    // will return `None`.
    while thread_id(thread).is_some() {
        let _ = rustix::thread::nanosleep(&rustix::thread::Timespec {
            tv_sec: 0,
            tv_nsec: 100_000_000,
        });
    }

    join_thread(thread);

    assert_eq!(current_thread_id(), thread_id(current_thread()).unwrap());
    exit(201);
}

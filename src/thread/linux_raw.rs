//! Thread startup and shutdown.
//!
//! Why does this api look like `thread::join(t)` instead of `t.join()`? Either
//! way could work, but free functions help emphasize that this API's
//! [`Thread`] differs from `std::thread::Thread`. It does not detach or free
//! its resources on drop, and does not guarantee validity. That gives users
//! more control when creating efficient higher-level abstractions like
//! pthreads or `std::thread::Thread`.

use crate::arch::{
    clone, munmap_and_exit_thread, set_thread_pointer, thread_pointer, STACK_ALIGNMENT, TLS_OFFSET,
};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "unstable-errno")]
use core::cell::Cell;
use core::cmp::max;
use core::ffi::c_void;
use core::mem::{align_of, size_of};
use core::ptr::{copy_nonoverlapping, drop_in_place, null, null_mut, NonNull};
use core::slice;
use core::sync::atomic::Ordering::SeqCst;
use core::sync::atomic::{AtomicI32, AtomicPtr, AtomicU8};
use linux_raw_sys::elf::*;
use memoffset::offset_of;
use rustix::io;
use rustix::mm::{mmap_anonymous, mprotect, MapFlags, MprotectFlags, ProtFlags};
use rustix::param::{linux_execfn, page_size};
use rustix::process::{getrlimit, Resource};
use rustix::runtime::{exe_phdrs, set_tid_address};
use rustix::thread::gettid;

/// An opaque pointer to a thread.
///
/// This type does not detach or free resources on drop. It just leaks the
/// thread. To detach or join, call [`detach`] or [`join`] explicitly.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Thread(NonNull<ThreadData>);

impl Thread {
    /// Convert to `Self` from a raw pointer that was returned from
    /// `Thread::to_raw`.
    #[inline]
    pub fn from_raw(raw: *mut c_void) -> Self {
        Self(NonNull::new(raw.cast()).unwrap())
    }

    /// Convert to `Self` from a raw non-null pointer that was returned from
    /// `Thread::to_raw_non_null`.
    #[inline]
    pub fn from_raw_non_null(raw: NonNull<c_void>) -> Self {
        Self(raw.cast())
    }

    /// Convert to a raw pointer from a `Self`.
    ///
    /// This value is guaranteed to uniquely identify a thread, while it is
    /// running. After a thread has exited, this value may be reused by new
    /// threads.
    #[inline]
    pub fn to_raw(self) -> *mut c_void {
        self.0.cast().as_ptr()
    }

    /// Convert to a raw non-null pointer from a `Self`.
    ///
    /// This value is guaranteed to uniquely identify a thread, while it is
    /// running. After a thread has exited, this value may be reused by new
    /// threads.
    #[inline]
    pub fn to_raw_non_null(self) -> NonNull<c_void> {
        self.0.cast()
    }
}

/// Data associated with a thread.
///
/// This is not `repr(C)` and not ABI-exposed.
struct ThreadData {
    thread_id: AtomicI32,
    #[cfg(feature = "unstable-errno")]
    errno_val: Cell<i32>,
    detached: AtomicU8,
    stack_addr: *mut c_void,
    stack_size: usize,
    guard_size: usize,
    map_size: usize,
    return_value: AtomicPtr<c_void>,

    // Support a few dtors before using dynamic allocation.
    #[cfg(feature = "alloc")]
    dtors: smallvec::SmallVec<[Box<dyn FnOnce()>; 4]>,
}

// Values for `ThreadData::detached`.
const INITIAL: u8 = 0;
const DETACHED: u8 = 1;
const ABANDONED: u8 = 2;

impl ThreadData {
    #[inline]
    fn new(stack_addr: *mut c_void, stack_size: usize, guard_size: usize, map_size: usize) -> Self {
        Self {
            thread_id: AtomicI32::new(0),
            #[cfg(feature = "unstable-errno")]
            errno_val: Cell::new(0),
            detached: AtomicU8::new(INITIAL),
            stack_addr,
            stack_size,
            guard_size,
            map_size,
            return_value: AtomicPtr::new(null_mut()),
            #[cfg(feature = "alloc")]
            dtors: smallvec::SmallVec::new(),
        }
    }
}

/// Metadata describing a thread.
#[repr(C)]
struct Metadata {
    /// Crate-internal fields. On platforms where TLS data goes after the
    /// ABI-exposed fields, we store our fields before them.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    thread: ThreadData,

    /// ABI-exposed fields. This is allocated at a platform-specific offset
    /// from the platform thread-pointer register value.
    abi: Abi,

    /// Crate-internal fields. On platforms where TLS data goes before the
    /// ABI-exposed fields, we store our fields after them.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    thread: ThreadData,
}

/// Fields which accessed by user code via well-known offsets from the platform
/// thread-pointer register. Specifically, the thread-pointer register points
/// to the `thread_pointee` field.
#[repr(C)]
#[cfg_attr(target_arch = "arm", repr(align(8)))]
struct Abi {
    /// The address the thread pointer points to.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    thread_pointee: [u8; 0],

    /// The ABI-exposed `canary` field.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    canary: usize,

    /// The address the thread pointer points to.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    thread_pointee: [u8; 0],

    /// The ABI-exposed `dtv` field (though we don't yet implement dynamic
    /// linking).
    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    dtv: *const c_void,

    /// The address the thread pointer points to.
    #[cfg(target_arch = "riscv64")]
    thread_pointee: [u8; 0],

    /// Padding to put the TLS data which follows at its well-known offset.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    _pad: [usize; 1],

    /// Padding to put the TLS data which follows at its well-known offset.
    #[cfg(target_arch = "riscv64")]
    _pad: [usize; 0],

    /// x86 and x86-64 put a copy of the thread-pointer register at the memory
    /// location pointed to by the thread-pointer register, because reading the
    /// thread-pointer register directly is slow.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    this: *mut c_void,

    /// The ABI-exposed `dtv` field (though we don't yet implement dynamic
    /// linking).
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    dtv: *const c_void,

    /// Padding to put the `canary` field at its well-known offset.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    _pad: [usize; 3],

    /// The ABI-exposed `canary` field.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    canary: usize,
}

/// Information obtained from the `DT_TLS` segment of the executable.
///
/// This variable must be initialized with [`initialize_startup_info`] before
/// use.
static mut STARTUP_TLS_INFO: StartupTlsInfo = StartupTlsInfo {
    addr: null(),
    mem_size: 0,
    file_size: 0,
    align: 0,
};

/// The type of [`STARTUP_TLS_INFO`].
///
/// This is not `repr(C)` and not ABI-exposed.
struct StartupTlsInfo {
    /// The base address of the TLS segment. Once initialize, this is
    /// always non-null, even when the TLS data is absent, so that the
    /// `addr` and `file_size` fields are suitable for passing to
    /// `slice::from_raw_parts`.
    addr: *const c_void,
    /// The size of the memory region pointed to by `addr`.
    mem_size: usize,
    /// From this offset up to `mem_size` is zero-initialized.
    file_size: usize,
    /// The required alignment for the TLS segment.
    align: usize,
}

/// The requested minimum size for stacks.
static mut STARTUP_STACK_SIZE: usize = 0;

/// Initialize `STARTUP_TLS_INFO` and `STARTUP_STACK_SIZE`.
///
/// Read values from the main executable segment headers (“phdrs”) relevant
/// to initializing TLS provided to the program at startup, and store them in
/// `STARTUP_TLS_INFO`.
pub(super) fn initialize_startup_info() {
    let mut tls_phdr = null();
    let mut stack_size = 0;
    let mut offset = 0;

    let (first_phdr, phent, phnum) = exe_phdrs();
    let mut current_phdr = first_phdr.cast::<Elf_Phdr>();

    // The dynamic address of the dynamic section, which we can compare with
    // the `PT_DYNAMIC` header's static address, if present.
    //
    // SAFETY: We're just taking the address of `_DYNAMIC` for arithmetic
    // purposes, not dereferencing it.
    let dynamic_addr: *const c_void = unsafe { &_DYNAMIC };

    // SAFETY: We assume that the phdr array pointer and length the kernel
    // provided to the process describe a valid phdr array, and that there are
    // no other threads running so we can store to `STARTUP_TLS_INFO` and
    // `STARTUP_STACK_SIZE` without synchronization.
    unsafe {
        let phdrs_end = current_phdr.cast::<u8>().add(phnum * phent).cast();
        while current_phdr != phdrs_end {
            let phdr = &*current_phdr;
            current_phdr = current_phdr.cast::<u8>().add(phent).cast();

            match phdr.p_type {
                // Compute the offset from the static virtual addresses in the
                // `p_vaddr` fields to the dynamic addresses. We don't always
                // get a `PT_PHDR` or `PT_DYNAMIC` header, so use whichever one
                // we get.
                PT_PHDR => offset = first_phdr.addr().wrapping_sub(phdr.p_vaddr),
                PT_DYNAMIC => offset = dynamic_addr.addr().wrapping_sub(phdr.p_vaddr),

                PT_TLS => tls_phdr = phdr,
                PT_GNU_STACK => stack_size = phdr.p_memsz,

                _ => {}
            }
        }

        STARTUP_TLS_INFO = if tls_phdr.is_null() {
            // No `PT_TLS` section. Assume an empty TLS.
            StartupTlsInfo {
                addr: NonNull::dangling().as_ptr(),
                mem_size: 0,
                file_size: 0,
                align: 1,
            }
        } else {
            // We saw a `PT_TLS` section. Initialize the fields.
            let tls_phdr = &*tls_phdr;
            StartupTlsInfo {
                addr: first_phdr.with_addr(offset.wrapping_add(tls_phdr.p_vaddr)),
                mem_size: tls_phdr.p_memsz,
                file_size: tls_phdr.p_filesz,
                align: tls_phdr.p_align,
            }
        };

        STARTUP_STACK_SIZE = stack_size;
    }
}

extern "C" {
    /// Declare the `_DYNAMIC` symbol so that we can compare its address with
    /// the static address in the `PT_DYNAMIC` header to learn our offset. Use
    /// a weak symbol because `_DYNAMIC` is not always present.
    static _DYNAMIC: c_void;
}
// Rust has `extern_weak` but it isn't stable, so use a `global_asm`.
core::arch::global_asm!(".weak _DYNAMIC");

/// A numerical thread identifier.
pub use rustix::thread::Pid as ThreadId;

/// Initialize the main thread.
///
/// This function is similar to `create_thread` except that the OS thread is
/// already created, and already has a stack (which we need to locate), and is
/// already running. We still need to create the thread [`Metadata`], copy in
/// the TLS initializers, and point the thread pointer to it so that it follows
/// the thread ABI that all the other threads follow.
///
/// # Safety
///
/// `initialize_startup_info` must be called before this. And `mem` must be the
/// initial value of the stack pointer in a new process, pointing to the
/// initial contents of the stack.
pub(super) unsafe fn initialize_main(mem: *mut c_void) {
    // Determine the top of the stack. Linux puts the `AT_EXECFN` string at
    // the top, so find the end of that, and then round up to the page size.
    // See <https://lwn.net/Articles/631631/> for details.
    let execfn = linux_execfn().to_bytes_with_nul();
    let stack_base = execfn.as_ptr().add(execfn.len());
    let stack_base = stack_base
        .map_addr(|ptr| round_up(ptr, page_size()))
        .cast_mut();

    // We're running before any user code, so the startup soft stack limit is
    // the effective stack size. Linux sets up inaccessible memory at the end
    // of the stack.
    let stack_map_size = getrlimit(Resource::Stack).current.unwrap() as usize;
    let stack_least = stack_base.cast::<u8>().sub(stack_map_size);
    let stack_size = stack_least.offset_from(mem.cast::<u8>()) as usize;
    let guard_size = page_size();

    // Initialize the canary value from the OS-provided random bytes.
    let random_ptr = rustix::runtime::random().cast::<usize>();
    let canary = random_ptr.read_unaligned();
    __stack_chk_guard = canary;

    let map_size = 0;

    // Compute relevant alignments.
    let tls_data_align = STARTUP_TLS_INFO.align;
    let header_align = align_of::<Metadata>();
    let metadata_align = max(tls_data_align, header_align);

    // Compute the size to allocate for thread data.
    let mut alloc_size = 0;

    // Variant II: TLS data goes below the TCB.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let tls_data_bottom = alloc_size;

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        alloc_size += round_up(STARTUP_TLS_INFO.mem_size, metadata_align);
    }

    let header = alloc_size;

    alloc_size += size_of::<Metadata>();

    // Variant I: TLS data goes above the TCB.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    {
        alloc_size = round_up(alloc_size, tls_data_align);
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    let tls_data_bottom = alloc_size;

    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    {
        alloc_size += round_up(STARTUP_TLS_INFO.mem_size, tls_data_align);
    }

    // Allocate the thread data. Use `mmap_anonymous` rather than `alloc` here
    // as the allocator may depend on thread-local data, which is what we're
    // initializing here.
    let new = mmap_anonymous(
        null_mut(),
        alloc_size,
        ProtFlags::READ | ProtFlags::WRITE,
        MapFlags::PRIVATE,
    )
    .unwrap()
    .cast::<u8>();
    debug_assert_eq!(new.addr() % metadata_align, 0);

    let tls_data = new.add(tls_data_bottom);
    let metadata: *mut Metadata = new.add(header).cast();
    let newtls: *mut c_void = (*metadata).abi.thread_pointee.as_mut_ptr().cast();

    // Initialize the thread metadata.
    metadata.write(Metadata {
        abi: Abi {
            canary,
            dtv: null(),
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            this: newtls,
            _pad: Default::default(),
            thread_pointee: [],
        },
        thread: ThreadData::new(stack_least.cast(), stack_size, guard_size, map_size),
    });

    // Initialize the TLS data with explicit initializer data.
    slice::from_raw_parts_mut(tls_data, STARTUP_TLS_INFO.file_size).copy_from_slice(
        slice::from_raw_parts(
            STARTUP_TLS_INFO.addr.cast::<u8>(),
            STARTUP_TLS_INFO.file_size,
        ),
    );

    // Initialize the TLS data beyond `file_size` which is zero-filled.
    slice::from_raw_parts_mut(
        tls_data.add(STARTUP_TLS_INFO.file_size),
        STARTUP_TLS_INFO.mem_size - STARTUP_TLS_INFO.file_size,
    )
    .fill(0);

    let thread_id_ptr = (*metadata).thread.thread_id.as_ptr();
    let tid = rustix::runtime::set_tid_address(thread_id_ptr.cast());
    *thread_id_ptr = tid.as_raw_nonzero().get();

    // Point the platform thread-pointer register at the new thread metadata.
    set_thread_pointer(newtls);
}

/// Creates a new thread.
///
/// `fn_(args)` is called on the new thread, except that the argument values
/// copied to memory that can be exclusively referenced by the thread.
///
/// # Safety
///
/// The values of `args` must be valid to send to the new thread, `fn_(args)`
/// on the new thread must have defined behavior, and the return value must be
/// valid to send to other threads.
pub unsafe fn create(
    fn_: unsafe fn(&mut [Option<NonNull<c_void>>]) -> Option<NonNull<c_void>>,
    args: &[Option<NonNull<c_void>>],
    stack_size: usize,
    guard_size: usize,
) -> io::Result<Thread> {
    // SAFETY: `STARTUP_TLS_INFO` is initialized at program startup before
    // we come here creating new threads.
    let (startup_tls_align, startup_tls_mem_size) =
        unsafe { (STARTUP_TLS_INFO.align, STARTUP_TLS_INFO.mem_size) };

    // Compute relevant alignments.
    let tls_data_align = startup_tls_align;
    let page_align = page_size();
    let stack_align = 16;
    let header_align = align_of::<Metadata>();
    let metadata_align = max(tls_data_align, header_align);
    let stack_metadata_align = max(stack_align, metadata_align);
    debug_assert!(stack_metadata_align <= page_align);

    // Compute the `mmap` size.
    let mut map_size = 0;

    map_size += round_up(guard_size, page_align);

    let stack_bottom = map_size;

    map_size += round_up(stack_size, stack_metadata_align);

    let stack_top = map_size;

    // Variant II: TLS data goes below the TCB.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let tls_data_bottom = map_size;

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        map_size += round_up(startup_tls_mem_size, tls_data_align);
    }

    let header = map_size;

    map_size += size_of::<Metadata>();

    // Variant I: TLS data goes above the TCB.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    {
        map_size = round_up(map_size, tls_data_align);
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    let tls_data_bottom = map_size;

    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    {
        map_size += round_up(startup_tls_mem_size, tls_data_align);
    }

    // Now we'll `mmap` the memory, initialize it, and create the OS thread.
    unsafe {
        // Allocate address space for the thread, including guard pages.
        let map = mmap_anonymous(
            null_mut(),
            map_size,
            ProtFlags::empty(),
            MapFlags::PRIVATE | MapFlags::STACK,
        )?
        .cast::<u8>();

        // Make the thread metadata and stack readable and writeable, leaving
        // the guard region inaccessible.
        mprotect(
            map.add(stack_bottom).cast(),
            map_size - stack_bottom,
            MprotectFlags::READ | MprotectFlags::WRITE,
        )?;

        // Compute specific pointers into the thread's memory.
        let stack = map.add(stack_top);
        let stack_least = map.add(stack_bottom);

        let tls_data = map.add(tls_data_bottom);
        let metadata: *mut Metadata = map.add(header).cast();
        let newtls: *mut c_void = (*metadata).abi.thread_pointee.as_mut_ptr().cast();

        // Copy the current thread's canary to the new thread.
        let canary = (*current_metadata()).abi.canary;

        // Initialize the thread metadata.
        metadata.write(Metadata {
            abi: Abi {
                canary,
                dtv: null(),
                #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                this: newtls,
                _pad: Default::default(),
                thread_pointee: [],
            },
            thread: ThreadData::new(stack_least.cast(), stack_size, guard_size, map_size),
        });

        // Initialize the TLS data with explicit initializer data.
        slice::from_raw_parts_mut(tls_data, STARTUP_TLS_INFO.file_size).copy_from_slice(
            slice::from_raw_parts(
                STARTUP_TLS_INFO.addr.cast::<u8>(),
                STARTUP_TLS_INFO.file_size,
            ),
        );

        // Allocate space for the thread arguments on the child's stack.
        let stack = stack.cast::<Option<NonNull<c_void>>>().sub(args.len());

        // Align the stack pointer.
        let stack = stack.with_addr(stack.addr() & STACK_ALIGNMENT.wrapping_neg());

        // Store the thread arguments on the child's stack.
        copy_nonoverlapping(args.as_ptr(), stack, args.len());

        // The TLS region includes additional data beyond `file_size` which is
        // expected to be zero-initialized, but we don't need to do anything
        // here since we allocated the memory with `mmap_anonymous` so it's
        // already zeroed.

        // Create the OS thread. In Linux, this is a process that shares much
        // of its state with the current process. We also pass additional
        // flags:
        //  - `SETTLS` to set the platform thread register.
        //  - `CHILD_CLEARTID` to arrange for a futex wait for threads waiting
        //    in `join_thread`.
        //  - `PARENT_SETTID` to store the child's tid at the `parent_tid`
        //    location.
        //  - `CHILD_SETTID` to store the child's tid at the `child_tid`
        //    location.
        // We receive the tid in the same memory for the parent and the child,
        // but we set both `PARENT_SETTID` and `CHILD_SETTID` to ensure that
        // the store completes before either the parent or child reads the tid.
        let flags = CloneFlags::VM
            | CloneFlags::FS
            | CloneFlags::FILES
            | CloneFlags::SIGHAND
            | CloneFlags::THREAD
            | CloneFlags::SYSVSEM
            | CloneFlags::SETTLS
            | CloneFlags::CHILD_CLEARTID
            | CloneFlags::CHILD_SETTID
            | CloneFlags::PARENT_SETTID;
        let thread_id_ptr = (*metadata).thread.thread_id.as_ptr();
        let clone_res = clone(
            flags.bits(),
            stack.cast(),
            thread_id_ptr,
            thread_id_ptr,
            newtls,
            core::mem::transmute(fn_),
            args.len(),
        );
        if clone_res >= 0 {
            #[cfg(feature = "log")]
            {
                let id = current_id();
                log::trace!(
                    "Thread[{:?}] launched thread Thread[{:?}] with stack_size={} and guard_size={}",
                    id.as_raw_nonzero(),
                    clone_res,
                    stack_size,
                    guard_size
                );
                for (i, arg) in args.iter().enumerate() {
                    log::trace!("Thread[{:?}] args[{}]: {:?}", id.as_raw_nonzero(), i, arg);
                }
            }

            Ok(Thread(NonNull::from(&mut (*metadata).thread)))
        } else {
            Err(io::Errno::from_raw_os_error(-clone_res as i32))
        }
    }
}

/// The entrypoint where Rust code is first executed on a new thread.
///
/// This transmutes `fn_` to
/// `unsafe fn(&mut [*mut c_void]) -> Option<NonNull<c_void>>` and then calls
/// it on the new thread. When `fn_` returns, the thread exits.
///
/// # Safety
///
/// `fn_` must be valid to transmute the function as described above and call
/// it in the new thread.
///
/// After calling `fn_`, this terminates the thread.
pub(super) unsafe extern "C" fn entry(
    fn_: extern "C" fn(),
    args: *mut *mut c_void,
    num_args: usize,
) -> ! {
    #[cfg(feature = "log")]
    log::trace!("Thread[{:?}] launched", current_id().as_raw_nonzero());

    // Do some basic precondition checks, to ensure that our assembly code did
    // what we expect it to do. These are debug-only for now, to keep the
    // release-mode startup code simple to disassemble and inspect, while we're
    // getting started.
    #[cfg(debug_assertions)]
    {
        extern "C" {
            #[link_name = "llvm.frameaddress"]
            fn builtin_frame_address(level: i32) -> *const u8;
            #[link_name = "llvm.returnaddress"]
            fn builtin_return_address(level: i32) -> *const u8;
            #[cfg(target_arch = "aarch64")]
            #[link_name = "llvm.sponentry"]
            fn builtin_sponentry() -> *const u8;
        }

        // Check that the incoming stack pointer is where we expect it to be.
        debug_assert_eq!(builtin_return_address(0), null());
        debug_assert_ne!(builtin_frame_address(0), null());
        #[cfg(not(any(target_arch = "x86", target_arch = "arm")))]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0xf, 0);
        #[cfg(target_arch = "arm")]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0x3, 0);
        #[cfg(target_arch = "x86")]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0xf, 8);
        debug_assert_eq!(builtin_frame_address(1), null());
        #[cfg(target_arch = "aarch64")]
        debug_assert_ne!(builtin_sponentry(), null());
        #[cfg(target_arch = "aarch64")]
        debug_assert_eq!(builtin_sponentry().addr() & 0xf, 0);

        // Check that `clone` stored our thread id as we expected.
        debug_assert_eq!(current_id(), gettid());
    }

    // Call the user thread function. In `std`, this is `thread_start`. Ignore
    // the return value for now, as `std` doesn't need it.
    let fn_: unsafe fn(&mut [*mut c_void]) -> Option<NonNull<c_void>> = core::mem::transmute(fn_);
    let args = slice::from_raw_parts_mut(args, num_args);
    let return_value = fn_(args);

    exit(return_value)
}

/// Call the destructors registered with [`at_exit`] and exit the thread.
unsafe fn exit(return_value: Option<NonNull<c_void>>) -> ! {
    let current = current();

    #[cfg(feature = "log")]
    if log::log_enabled!(log::Level::Trace) {
        log::trace!(
            "Thread[{:?}] returned {:?}",
            current.0.as_ref().thread_id.load(SeqCst),
            return_value
        );
    }

    // Call functions registered with `at_exit`.
    #[cfg(feature = "alloc")]
    call_dtors(current);

    // Read the thread's state, and set it to `ABANDONED` if it was `INITIAL`,
    // which tells `join_thread` to free the memory. Otherwise, it's in the
    // `DETACHED` state, and we free the memory immediately.
    let state = current
        .0
        .as_ref()
        .detached
        .compare_exchange(INITIAL, ABANDONED, SeqCst, SeqCst);
    if let Err(e) = state {
        // The thread was detached. Prepare to free the memory. First read out
        // all the fields that we'll need before freeing it.
        #[cfg(feature = "log")]
        let current_thread_id = current.0.as_ref().thread_id.load(SeqCst);
        let current_map_size = current.0.as_ref().map_size;
        let current_stack_addr = current.0.as_ref().stack_addr;
        let current_guard_size = current.0.as_ref().guard_size;

        #[cfg(feature = "log")]
        log::trace!("Thread[{:?}] exiting as detached", current_thread_id);
        debug_assert_eq!(e, DETACHED);

        // Deallocate the `ThreadData`.
        drop_in_place(current.0.as_ptr());

        // Free the thread's `mmap` region, if we allocated it.
        let map_size = current_map_size;
        if map_size != 0 {
            // Null out the tid address so that the kernel doesn't write to
            // memory that we've freed trying to clear our tid when we exit.
            let _ = set_tid_address(null_mut());

            // `munmap` the memory, which also frees the stack we're currently
            // on, and do an `exit` carefully without touching the stack.
            let map = current_stack_addr.cast::<u8>().sub(current_guard_size);
            munmap_and_exit_thread(map.cast(), map_size);
        }
    } else {
        // The thread was not detached, so its memory will be freed when it's
        // joined.
        #[cfg(feature = "log")]
        if log::log_enabled!(log::Level::Trace) {
            log::trace!(
                "Thread[{:?}] exiting as joinable",
                current.0.as_ref().thread_id.load(SeqCst)
            );
        }

        // Convert `return_value` into a `*mut c_void` so that we can store it
        // in an `AtomicPtr`.
        let return_value = match return_value {
            Some(return_value) => return_value.as_ptr(),
            None => null_mut(),
        };

        // Store the return value in the thread for `join_thread` to read.
        current.0.as_ref().return_value.store(return_value, SeqCst);
    }

    // Terminate the thread.
    rustix::runtime::exit_thread(0)
}

/// Call the destructors registered with [`at_exit`].
#[cfg(feature = "alloc")]
pub(crate) fn call_dtors(current: Thread) {
    let mut current = current;

    // Run the `dtors`, in reverse order of registration. Note that destructors
    // may register new destructors.
    //
    // SAFETY: `current` points to thread-local data which is valid as long as
    // the thread is alive.
    while let Some(func) = unsafe { current.0.as_mut().dtors.pop() } {
        #[cfg(feature = "log")]
        if log::log_enabled!(log::Level::Trace) {
            log::trace!(
                "Thread[{:?}] calling `thread::at_exit`-registered function",
                unsafe { current.0.as_ref().thread_id.load(SeqCst) },
            );
        }

        func();
    }
}

/// Marks a thread as “detached”.
///
/// Detached threads free their own resources automatically when they
/// exit, rather than when they are joined.
///
/// # Safety
///
/// `thread` must point to a valid thread record that has not yet been
/// detached and will not be joined.
#[inline]
pub unsafe fn detach(thread: Thread) {
    #[cfg(feature = "log")]
    let thread_id = thread.0.as_ref().thread_id.load(SeqCst);

    #[cfg(feature = "log")]
    if log::log_enabled!(log::Level::Trace) {
        log::trace!(
            "Thread[{:?}] marked as detached by Thread[{:?}]",
            thread_id,
            current_id().as_raw_nonzero()
        );
    }

    if thread.0.as_ref().detached.swap(DETACHED, SeqCst) == ABANDONED {
        wait_for_exit(thread);

        #[cfg(feature = "log")]
        log_thread_to_be_freed(thread_id);

        free_memory(thread);
    }
}

/// Waits for a thread to finish.
///
/// The return value is the value returned from the call to the `fn_` passed to
/// `create_thread`.
///
/// # Safety
///
/// `thread` must point to a valid thread record that has not already been
/// detached or joined.
pub unsafe fn join(thread: Thread) -> Option<NonNull<c_void>> {
    let thread_data = thread.0.as_ref();

    #[cfg(feature = "log")]
    let thread_id = thread_data.thread_id.load(SeqCst);

    #[cfg(feature = "log")]
    if log::log_enabled!(log::Level::Trace) {
        log::trace!(
            "Thread[{:?}] is being joined by Thread[{:?}]",
            thread_id,
            current_id().as_raw_nonzero()
        );
    }

    wait_for_exit(thread);
    debug_assert_eq!(thread_data.detached.load(SeqCst), ABANDONED);

    #[cfg(feature = "log")]
    log_thread_to_be_freed(thread_id);

    // Load the return value stored by `exit_thread`, before we free the
    // thread's memory.
    let return_value = thread_data.return_value.load(SeqCst);

    // `munmap` the stack and metadata for the thread.
    free_memory(thread);

    // Convert the `*mut c_void` we stored in the `AtomicPtr` back into an
    // `Option<NonNull<c_void>>`.
    NonNull::new(return_value)
}

/// Wait until `thread` has exited.
///
/// `thread` must point to a valid thread record that has not already been
/// detached or joined.
unsafe fn wait_for_exit(thread: Thread) {
    use rustix::thread::{futex, FutexFlags, FutexOperation};

    // Check whether the thread has exited already; we set the
    // `CloneFlags::CHILD_CLEARTID` flag on the clone syscall, so we can test
    // for `NONE` here.
    let thread_data = thread.0.as_ref();
    let thread_id = &thread_data.thread_id;
    while let Some(id_value) = ThreadId::from_raw(thread_id.load(SeqCst)) {
        // This doesn't use any shared memory, but we can't use
        // `FutexFlags::PRIVATE` because the wake comes from Linux
        // as arranged by the `CloneFlags::CHILD_CLEARTID` flag,
        // and Linux doesn't use the private flag for the wake.
        match futex(
            thread_id.as_ptr().cast::<u32>(),
            FutexOperation::Wait,
            FutexFlags::empty(),
            id_value.as_raw_nonzero().get() as u32,
            null(),
            null_mut(),
            0,
        ) {
            Ok(_) => break,
            Err(io::Errno::INTR) => continue,
            Err(e) => debug_assert_eq!(e, io::Errno::AGAIN),
        }
    }
}

#[cfg(feature = "log")]
fn log_thread_to_be_freed(thread_id: i32) {
    if log::log_enabled!(log::Level::Trace) {
        log::trace!("Thread[{:?}] memory being freed", thread_id);
    }
}

/// Free any dynamically-allocated memory for `thread`.
///
/// # Safety
///
/// `thread` must point to a valid thread record for a thread that has
/// already exited.
unsafe fn free_memory(thread: Thread) {
    use rustix::mm::munmap;

    // The thread was detached. Prepare to free the memory. First read out
    // all the fields that we'll need before freeing it.
    let map_size = thread.0.as_ref().map_size;
    let stack_addr = thread.0.as_ref().stack_addr;
    let guard_size = thread.0.as_ref().guard_size;

    // Deallocate the `ThreadData`.
    drop_in_place(thread.0.as_ptr());

    // Free the thread's `mmap` region, if we allocated it.
    if map_size != 0 {
        let map = stack_addr.cast::<u8>().sub(guard_size);
        munmap(map.cast(), map_size).unwrap();
    }
}

/// Registers a function to call when the current thread exits.
#[cfg(feature = "alloc")]
pub fn at_exit(func: Box<dyn FnOnce()>) {
    // SAFETY: `current()` points to thread-local data which is valid as long
    // as the thread is alive.
    unsafe {
        current().0.as_mut().dtors.push(func);
    }
}

#[inline]
#[must_use]
fn current_metadata() -> *mut Metadata {
    thread_pointer()
        .cast::<u8>()
        .wrapping_sub(offset_of!(Metadata, abi) + offset_of!(Abi, thread_pointee))
        .cast()
}

/// Return a raw pointer to the data associated with the current thread.
#[inline]
#[must_use]
pub fn current() -> Thread {
    // SAFETY: This is only called after we've initialized all the thread
    // state.
    unsafe { Thread(NonNull::from(&mut (*current_metadata()).thread)) }
}

/// Return the current thread id.
///
/// This is the same as [`rustix::thread::gettid`], but loads the value from a
/// field in the runtime rather than making a system call.
#[inline]
#[must_use]
pub fn current_id() -> ThreadId {
    // Don't use the `id` function here because it returns an `Option` to
    // handle the case where the thread has exited. We're querying the current
    // thread which we know is still running because we're on it.
    //
    // SAFETY: All threads have been initialized, including the main thread
    // with `initialize_main`, so `current()` returns a valid pointer.
    let tid = unsafe { ThreadId::from_raw_unchecked(current().0.as_ref().thread_id.load(SeqCst)) };
    debug_assert_eq!(tid, gettid(), "`current_id` disagrees with `gettid`");
    tid
}

/// Set the current thread id, after a `fork`.
///
/// The only valid use for this is in the implementation of libc-like `fork`
/// wrappers such as the one in c-scape. `posix_spawn`-like uses of `fork`
/// don't need to do this because they shouldn't do anything that cares about
/// the thread id before doing their `execve`.
///
/// # Safety
///
/// This must only be called immediately after a `fork` before any other
/// threads are created. `tid` must be the same value as what [`gettid`] would
/// return.
#[doc(hidden)]
#[inline]
pub unsafe fn set_current_id_after_a_fork(tid: ThreadId) {
    let current = current();
    debug_assert_ne!(
        tid.as_raw_nonzero().get(),
        current.0.as_ref().thread_id.load(SeqCst),
        "current thread ID already matches new thread ID"
    );
    debug_assert_eq!(tid, gettid(), "new thread ID disagrees with `gettid`");
    current
        .0
        .as_ref()
        .thread_id
        .store(tid.as_raw_nonzero().get(), SeqCst);
}

/// Return the address of the thread-local `errno` state.
///
/// This is equivalent to `__errno_location()` in glibc and musl.
#[cfg(feature = "unstable-errno")]
#[inline]
pub fn errno_location() -> *mut i32 {
    unsafe { core::ptr::addr_of_mut!((*current_metadata()).thread.errno_val).cast::<i32>() }
}

/// Return the TLS address for the given `module` and `offset` for the current
/// thread.
#[inline]
#[must_use]
pub fn current_tls_addr(module: usize, offset: usize) -> *mut c_void {
    // Offset 0 is the generation field, and we don't support dynamic linking,
    // so we should only ever see 1 here.
    assert_eq!(module, 1);

    // Platforms where TLS data goes after the ABI-exposed fields.
    #[cfg(any(target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64"))]
    {
        thread_pointer()
            .cast::<u8>()
            .wrapping_add(size_of::<Abi>() - offset_of!(Abi, thread_pointee))
            .wrapping_add(TLS_OFFSET)
            .wrapping_add(offset)
            .cast()
    }

    // Platforms where TLS data goes before the ABI-exposed fields.
    //
    // SAFETY: `STARTUP_TLS_INFO` has already been initialized by
    // [`initialize_startup_info`].
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    unsafe {
        thread_pointer()
            .cast::<u8>()
            .wrapping_sub(STARTUP_TLS_INFO.mem_size)
            .wrapping_add(TLS_OFFSET)
            .wrapping_add(offset)
            .cast()
    }
}

/// Return the id of a thread, or `None` if the thread has exited.
///
/// This is the same as [`rustix::thread::gettid`], but loads the value from a
/// field in the runtime rather than making a system call.
///
/// # Safety
///
/// `thread` must point to a valid thread record.
#[inline]
pub unsafe fn id(thread: Thread) -> Option<ThreadId> {
    let raw = thread.0.as_ref().thread_id.load(SeqCst);
    ThreadId::from_raw(raw)
}

/// Return the current thread's stack address (lowest address), size, and guard
/// size.
///
/// # Safety
///
/// `thread` must point to a valid thread record.
#[inline]
#[must_use]
pub unsafe fn stack(thread: Thread) -> (*mut c_void, usize, usize) {
    let data = thread.0.as_ref();
    (data.stack_addr, data.stack_size, data.guard_size)
}

/// Return the default stack size for new threads.
#[inline]
#[must_use]
pub fn default_stack_size() -> usize {
    // This is just something simple that works for now.
    //
    // SAFETY: `STARTUP_STACK_SIZE` has already been initialized by
    // [`initialize_startup_info`].
    unsafe { max(0x20000, STARTUP_STACK_SIZE) }
}

/// Return the default guard size for new threads.
#[inline]
#[must_use]
pub fn default_guard_size() -> usize {
    // This is just something simple that works for now.
    page_size() * 4
}

/// Yield the current thread, encouraging other threads to run.
#[inline]
pub fn yield_current() {
    rustix::process::sched_yield()
}

/// The ARM ABI expects this to be defined.
#[cfg(target_arch = "arm")]
#[no_mangle]
extern "C" fn __aeabi_read_tp() -> *mut c_void {
    thread_pointer()
}

/// Some targets use this global variable instead of the TLS `canary` field.
#[no_mangle]
static mut __stack_chk_guard: usize = 0;

const fn round_up(addr: usize, boundary: usize) -> usize {
    (addr + (boundary - 1)) & boundary.wrapping_neg()
}

// We define `clone` and `CloneFlags` here in `origin` instead of `rustix`
// because `clone` needs custom assembly code that knows about what we're
// using it for.
bitflags::bitflags! {
    struct CloneFlags: u32 {
        const NEWTIME        = linux_raw_sys::general::CLONE_NEWTIME; // since Linux 5.6
        const VM             = linux_raw_sys::general::CLONE_VM;
        const FS             = linux_raw_sys::general::CLONE_FS;
        const FILES          = linux_raw_sys::general::CLONE_FILES;
        const SIGHAND        = linux_raw_sys::general::CLONE_SIGHAND;
        const PIDFD          = linux_raw_sys::general::CLONE_PIDFD; // since Linux 5.2
        const PTRACE         = linux_raw_sys::general::CLONE_PTRACE;
        const VFORK          = linux_raw_sys::general::CLONE_VFORK;
        const PARENT         = linux_raw_sys::general::CLONE_PARENT;
        const THREAD         = linux_raw_sys::general::CLONE_THREAD;
        const NEWNS          = linux_raw_sys::general::CLONE_NEWNS;
        const SYSVSEM        = linux_raw_sys::general::CLONE_SYSVSEM;
        const SETTLS         = linux_raw_sys::general::CLONE_SETTLS;
        const PARENT_SETTID  = linux_raw_sys::general::CLONE_PARENT_SETTID;
        const CHILD_CLEARTID = linux_raw_sys::general::CLONE_CHILD_CLEARTID;
        const DETACHED       = linux_raw_sys::general::CLONE_DETACHED;
        const UNTRACED       = linux_raw_sys::general::CLONE_UNTRACED;
        const CHILD_SETTID   = linux_raw_sys::general::CLONE_CHILD_SETTID;
        const NEWCGROUP      = linux_raw_sys::general::CLONE_NEWCGROUP; // since Linux 4.6
        const NEWUTS         = linux_raw_sys::general::CLONE_NEWUTS;
        const NEWIPC         = linux_raw_sys::general::CLONE_NEWIPC;
        const NEWUSER        = linux_raw_sys::general::CLONE_NEWUSER;
        const NEWPID         = linux_raw_sys::general::CLONE_NEWPID;
        const NEWNET         = linux_raw_sys::general::CLONE_NEWNET;
        const IO             = linux_raw_sys::general::CLONE_IO;
    }
}

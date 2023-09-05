//! Program startup and shutdown.

#[cfg(feature = "origin-thread")]
use crate::thread::initialize_main_thread;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(all(
    feature = "alloc",
    any(feature = "origin-start", feature = "external-start")
))]
use alloc::vec::Vec;
#[cfg(all(
    feature = "init-fini-arrays",
    any(feature = "origin-start", feature = "external-start")
))]
use core::arch::asm;
#[cfg(not(any(feature = "origin-start", feature = "external-start")))]
use core::ptr::null_mut;
use linux_raw_sys::ctypes::c_int;
#[cfg(all(
    feature = "alloc",
    any(feature = "origin-start", feature = "external-start")
))]
use rustix_futex_sync::Mutex;

/// The entrypoint where Rust code is first executed when the program starts.
///
/// # Safety
///
/// `mem` should point to the stack as provided by the operating system.
#[cfg(any(feature = "origin-start", feature = "external-start"))]
pub(super) unsafe extern "C" fn entry(mem: *mut usize) -> ! {
    // Do some basic precondition checks, to ensure that our assembly code did
    // what we expect it to do. These are debug-only for now, to keep the
    // release-mode startup code simple to disassemble and inspect, while we're
    // getting started.
    #[cfg(debug_assertions)]
    #[cfg(feature = "origin-start")]
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

        // Check that `mem` is where we expect it to be.
        debug_assert_ne!(mem, core::ptr::null_mut());
        debug_assert_eq!(mem.addr() & 0xf, 0);
        debug_assert!(builtin_frame_address(0).addr() <= mem.addr());

        // Check that the incoming stack pointer is where we expect it to be.
        debug_assert_eq!(builtin_return_address(0), core::ptr::null());
        debug_assert_ne!(builtin_frame_address(0), core::ptr::null());
        #[cfg(not(any(target_arch = "arm", target_arch = "x86")))]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0xf, 0);
        #[cfg(target_arch = "arm")]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0x7, 0);
        #[cfg(target_arch = "x86")]
        debug_assert_eq!(builtin_frame_address(0).addr() & 0xf, 8);
        debug_assert_eq!(builtin_frame_address(1), core::ptr::null());
        #[cfg(target_arch = "aarch64")]
        debug_assert_ne!(builtin_sponentry(), core::ptr::null());
        #[cfg(target_arch = "aarch64")]
        debug_assert_eq!(builtin_sponentry().addr() & 0xf, 0);
    }

    // Compute `argc`, `argv`, and `envp`.
    let (argc, argv, envp) = compute_args(mem);
    init_runtime(mem, envp);

    let status = call_user_code(argc, argv, envp);

    // Run functions registered with `at_exit`, and exit with main's return
    // value.
    exit(status)
}

#[cfg(any(feature = "origin-start", feature = "external-start"))]
unsafe fn compute_args(mem: *mut usize) -> (i32, *mut *mut u8, *mut *mut u8) {
    use linux_raw_sys::ctypes::c_uint;

    let argc = *mem as c_int;
    let argv = mem.add(1).cast::<*mut u8>();
    let envp = argv.add(argc as c_uint as usize + 1);

    // Do a few more precondition checks on `argc` and `argv`.
    debug_assert!(argc >= 0);
    debug_assert_eq!(*mem, argc as _);
    debug_assert_eq!(*argv.add(argc as usize), core::ptr::null_mut());

    (argc, argv, envp)
}

#[cfg(any(feature = "origin-start", feature = "external-start"))]
#[allow(unused_variables)]
unsafe fn init_runtime(mem: *mut usize, envp: *mut *mut u8) {
    // Explicitly initialize `rustix` so that we can control the initialization
    // order.
    #[cfg(feature = "param")]
    rustix::param::init(envp);

    // After initializing the AUX data in rustix, but before doing anything
    // else, perform dynamic relocations.
    #[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
    #[cfg(relocation_model = "pic")]
    relocate();

    // Initialize the main thread.
    #[cfg(feature = "origin-thread")]
    initialize_main_thread(mem.cast());
}

#[cfg(any(feature = "origin-start", feature = "external-start"))]
#[allow(unused_variables)]
unsafe fn call_user_code(argc: c_int, argv: *mut *mut u8, envp: *mut *mut u8) -> i32 {
    extern "C" {
        fn main(argc: c_int, argv: *mut *mut u8, envp: *mut *mut u8) -> c_int;
    }

    // Call the functions registered via `.init_array`.
    #[cfg(feature = "init-fini-arrays")]
    call_ctors(argc, argv, envp);

    #[cfg(feature = "log")]
    log::trace!("Calling `main({:?}, {:?}, {:?})`", argc, argv, envp);

    // Call `main`.
    let status = main(argc, argv, envp);

    #[cfg(feature = "log")]
    log::trace!("`main` returned `{:?}`", status);

    status
}

/// Call the constructors in the `.init_array` section.
#[cfg(any(feature = "origin-start", feature = "external-start"))]
#[cfg(feature = "init-fini-arrays")]
unsafe fn call_ctors(argc: c_int, argv: *mut *mut u8, envp: *mut *mut u8) {
    use core::ffi::c_void;

    extern "C" {
        static __init_array_start: c_void;
        static __init_array_end: c_void;
    }

    // Call the `.init_array` functions. As GLIBC, does, pass argc, argv,
    // and envp as extra arguments. In addition to GLIBC ABI compatibility,
    // c-scape relies on this.
    type InitFn = fn(c_int, *mut *mut u8, *mut *mut u8);
    let mut init = &__init_array_start as *const _ as *const InitFn;
    let init_end = &__init_array_end as *const _ as *const InitFn;
    // Prevent the optimizer from optimizing the `!=` comparison to true;
    // `init` and `init_start` may have the same address.
    asm!("# {}", inout(reg) init);

    while init != init_end {
        #[cfg(feature = "log")]
        log::trace!(
            "Calling `.init_array`-registered function `{:?}({:?}, {:?}, {:?})`",
            *init,
            argc,
            argv,
            envp
        );
        (*init)(argc, argv, envp);
        init = init.add(1);
    }
}

/// Functions registered with [`at_exit`].
#[cfg(all(
    feature = "alloc",
    any(feature = "origin-start", feature = "external-start")
))]
static DTORS: Mutex<Vec<Box<dyn FnOnce() + Send>>> = Mutex::new(Vec::new());

/// Register a function to be called when [`exit`] is called.
///
/// # Safety
///
/// This arranges for `func` to be called, and passed `obj`, when the program
/// exits.
#[cfg(feature = "alloc")]
pub fn at_exit(func: Box<dyn FnOnce() + Send>) {
    #[cfg(any(feature = "origin-start", feature = "external-start"))]
    {
        let mut funcs = DTORS.lock();
        funcs.push(func);
    }

    #[cfg(not(any(feature = "origin-start", feature = "external-start")))]
    {
        use core::ffi::c_void;

        extern "C" {
            // <https://refspecs.linuxbase.org/LSB_3.1.0/LSB-generic/LSB-generic/baselib---cxa-atexit.html>
            fn __cxa_atexit(
                func: unsafe extern "C" fn(*mut c_void),
                arg: *mut c_void,
                _dso: *mut c_void,
            ) -> c_int;
        }

        // The function to pass to `__cxa_atexit`.
        unsafe extern "C" fn at_exit_func(arg: *mut c_void) {
            Box::from_raw(arg as *mut Box<dyn FnOnce() + Send>)();
        }

        let at_exit_arg = Box::into_raw(Box::new(func)).cast::<c_void>();
        let r = unsafe { __cxa_atexit(at_exit_func, at_exit_arg, null_mut()) };
        assert_eq!(r, 0);
    }
}

/// Call all the functions registered with [`at_exit`] or with the
/// `.fini_array` section, and exit the program.
pub fn exit(status: c_int) -> ! {
    // Call functions registered with `at_thread_exit`.
    #[cfg(all(
        feature = "thread",
        any(feature = "origin-start", feature = "external-start")
    ))]
    crate::thread::call_thread_dtors(crate::thread::current_thread());

    // Call all the registered functions, in reverse order. Leave `DTORS`
    // unlocked while making the call so that functions can add more functions
    // to the end of the list.
    #[cfg(all(
        feature = "alloc",
        any(feature = "origin-start", feature = "external-start")
    ))]
    loop {
        let mut dtors = DTORS.lock();
        if let Some(dtor) = dtors.pop() {
            #[cfg(feature = "log")]
            log::trace!("Calling `at_exit`-registered function");

            dtor();
        } else {
            break;
        }
    }

    // Call the `.fini_array` functions, in reverse order.
    #[cfg(any(feature = "origin-start", feature = "external-start"))]
    #[cfg(feature = "init-fini-arrays")]
    unsafe {
        use core::ffi::c_void;

        extern "C" {
            static __fini_array_start: c_void;
            static __fini_array_end: c_void;
        }

        type InitFn = fn();
        let mut fini: *const InitFn = &__fini_array_end as *const _ as *const InitFn;
        let fini_start: *const InitFn = &__fini_array_start as *const _ as *const InitFn;
        // Prevent the optimizer from optimizing the `!=` comparison to true;
        // `fini` and `fini_start` may have the same address.
        asm!("# {}", inout(reg) fini);

        while fini != fini_start {
            fini = fini.sub(1);

            #[cfg(feature = "log")]
            log::trace!("Calling `.fini_array`-registered function `{:?}()`", *fini);

            (*fini)();
        }
    }

    #[cfg(any(feature = "origin-start", feature = "external-start"))]
    {
        // Call `exit_immediately` to exit the program.
        exit_immediately(status)
    }

    #[cfg(not(any(feature = "origin-start", feature = "external-start")))]
    unsafe {
        // Call `libc` to run *its* dtors, and exit the program.
        libc::exit(status)
    }
}

/// Exit the program without calling functions registered with [`at_exit`] or
/// with the `.fini_array` section.
#[inline]
pub fn exit_immediately(status: c_int) -> ! {
    #[cfg(feature = "log")]
    log::trace!("Program exiting");

    #[cfg(any(feature = "origin-start", feature = "external-start"))]
    {
        // Call `rustix` to exit the program.
        rustix::runtime::exit_group(status)
    }

    #[cfg(not(any(feature = "origin-start", feature = "external-start")))]
    unsafe {
        // Call `libc` to exit the program.
        libc::_exit(status)
    }
}

/// Locate the dynamic (startup-time) relocations and perform them.
///
/// This is unsafer than unsafe. It's meant to be called at a time when Rust
/// code is running but before relocations have been performed. There are no
/// guarantees that this code won't segfault at any moment.
///
/// We use volatile accesses and `asm` optimization barriers to try to
/// discourage optimizers from thinking they understand what's happening, to
/// increase the probability of this code working.
///
/// And if this code panics, the panic code will probably segfault, because
/// `core::fmt` is known to use an address that needs relocation.
///
/// So yes, there's a reason this code is behind a feature flag.
#[cfg(all(feature = "experimental-relocate", feature = "origin-start"))]
#[cfg(relocation_model = "pic")]
#[cold]
unsafe fn relocate() {
    use core::arch::asm;
    use core::ffi::c_void;
    use core::mem::size_of;
    use core::ptr::{
        from_exposed_addr, from_exposed_addr_mut, read_volatile, write_volatile, NonNull,
    };
    use core::slice::from_raw_parts;
    use rustix::mm::{mprotect, MprotectFlags};
    use rustix::param::page_size;

    // Please do not take any of the following code as an example for how to
    // write Rust code in general.

    // Compute the static address of `_start`. This relies on the sneaky fact
    // that we initialize a static variable with the address of `_start`, and
    // if we haven't performed relocations yet, we'll be able to see the static
    // address. Also, the program *just* started so there are no other threads
    // yet, so loading from static memory without synchronization is fine. But
    // we still use volatile and `asm` to do everything we can to protect this
    // code because the optimizer won't have any idea what we're up to.
    struct StaticStart(*const u8);
    unsafe impl Sync for StaticStart {}
    static STATIC_START: StaticStart = StaticStart(crate::_start as *const u8);
    let mut static_start_addr: *const *const u8 = &STATIC_START.0;
    asm!("# {}", inout(reg) static_start_addr);
    let mut static_start = read_volatile(static_start_addr).addr();
    asm!("# {}", inout(reg) static_start);

    // Obtain the dynamic address of `_start` from the AUX records.
    let dynamic_start = rustix::runtime::entry();

    // Our offset is the difference between these two.
    let offset = dynamic_start.wrapping_sub(static_start);

    // This code doesn't rely on the offset being page aligned, but it is
    // useful to check to make sure we computed it correctly.
    debug_assert_eq!(offset & (page_size() - 1), 0);

    // If we're loaded at our static address, then there's nothing to do.
    if offset == 0 {
        return;
    }

    // Now, obtain the static Phdrs, which have been mapped into the address
    // space at an address provided to us in the AUX array.
    let (first_phdr, phent, phnum) = rustix::runtime::exe_phdrs();
    let mut current_phdr = first_phdr.cast::<Elf_Phdr>();

    // Next, look through the Phdrs to find the Dynamic section and the relro
    // description if present. In the `Dynamic` section, find the relocations
    // and perform them.
    let mut relro = 0;
    let mut relro_len = 0;
    let phdrs_end = current_phdr.cast::<u8>().add(phnum * phent).cast();
    while current_phdr != phdrs_end {
        let phdr = &*current_phdr;
        current_phdr = current_phdr.cast::<u8>().add(phent).cast();

        match phdr.p_type {
            PT_DYNAMIC => {
                // We found the dynamic section.
                let dynamic = from_exposed_addr(phdr.p_vaddr.wrapping_add(offset));
                let num_dyn = phdr.p_memsz / size_of::<Elf_Dyn>();
                let dyns: &[Elf_Dyn] = from_raw_parts(dynamic, num_dyn);

                // Look through the `Elf_Dyn` entries to find the location of
                // the relocation table.
                let mut rela_ptr = NonNull::dangling().as_ptr() as *const Elf_Rela;
                let mut num_rela = 0;
                let mut rela_ent_size = 0;
                for dyn_ in dyns {
                    match dyn_.d_tag as u32 {
                        DT_RELA => {
                            rela_ptr = from_exposed_addr(dyn_.d_un.d_ptr.wrapping_add(offset))
                        }
                        DT_RELASZ => num_rela = dyn_.d_un.d_val as usize / size_of::<Elf_Rela>(),
                        DT_RELAENT => rela_ent_size = dyn_.d_un.d_val as usize,
                        _ => (),
                    }
                }

                // Now, perform the relocations. As above, the optimizer won't
                // have any idea what we're up to, so use volatile and `asm`.
                let rela_end = rela_ptr.cast::<u8>().add(num_rela * rela_ent_size).cast();
                while rela_ptr != rela_end {
                    let rela = &*rela_ptr;
                    rela_ptr = rela_ptr.cast::<u8>().add(rela_ent_size).cast();

                    let addr: *mut c_void =
                        from_exposed_addr_mut(rela.r_offset.wrapping_add(offset));

                    match rela.type_() {
                        R_RELATIVE => {
                            let mut reloc_addr = addr.cast();
                            let mut reloc_value = rela.r_addend.wrapping_add(offset);
                            asm!("# {} {}", inout(reg) reloc_addr, inout(reg) reloc_value);
                            write_volatile(reloc_addr, reloc_value);
                            asm!("");
                        }
                        _ => unimplemented!(),
                    }
                }
            }
            PT_GNU_RELRO => {
                // A relro description is present. Make a note of it so that we
                // can mark memory readonly after relocations are done.
                relro = phdr.p_vaddr;
                relro_len = phdr.p_memsz;
            }
            _ => (),
        }
    }

    // Check that relocation did its job.
    #[cfg(debug_assertions)]
    {
        let mut static_start_addr: *const *const u8 = &STATIC_START.0;
        asm!("# {}", inout(reg) static_start_addr);
        let mut static_start = read_volatile(static_start_addr).addr();
        asm!("# {}", inout(reg) static_start);
        assert_eq!(static_start, dynamic_start);
    }

    // If we saw a relro description, mark the memory readonly.
    if relro_len != 0 {
        let mprotect_addr =
            from_exposed_addr_mut(relro.wrapping_add(offset) & page_size().wrapping_neg());
        mprotect(mprotect_addr, relro_len, MprotectFlags::READ).unwrap();
    }

    // ELF ABI.

    #[cfg(target_pointer_width = "32")]
    #[repr(C)]
    struct Elf_Phdr {
        p_type: u32,
        p_offset: usize,
        p_vaddr: usize,
        p_paddr: usize,
        p_filesz: usize,
        p_memsz: usize,
        p_flags: u32,
        p_align: usize,
    }

    #[cfg(target_pointer_width = "64")]
    #[repr(C)]
    struct Elf_Phdr {
        p_type: u32,
        p_flags: u32,
        p_offset: usize,
        p_vaddr: usize,
        p_paddr: usize,
        p_filesz: usize,
        p_memsz: usize,
        p_align: usize,
    }

    #[cfg(target_pointer_width = "32")]
    #[repr(C)]
    struct Elf_Dyn {
        d_tag: i32,
        d_un: Elf_Dyn_Union,
    }

    #[cfg(target_pointer_width = "32")]
    #[repr(C)]
    union Elf_Dyn_Union {
        d_val: u32,
        d_ptr: usize,
    }

    #[cfg(target_pointer_width = "64")]
    #[repr(C)]
    struct Elf_Dyn {
        d_tag: i64,
        d_un: Elf_Dyn_Union,
    }

    #[cfg(target_pointer_width = "64")]
    #[repr(C)]
    union Elf_Dyn_Union {
        d_val: u64,
        d_ptr: usize,
    }

    #[cfg(target_pointer_width = "32")]
    #[repr(C)]
    struct Elf_Rela {
        r_offset: usize,
        r_info: u32,
        r_addend: usize,
    }

    #[cfg(target_pointer_width = "64")]
    #[repr(C)]
    struct Elf_Rela {
        r_offset: usize,
        r_info: u64,
        r_addend: usize,
    }

    impl Elf_Rela {
        #[inline]
        fn type_(&self) -> u32 {
            #[cfg(target_pointer_width = "32")]
            {
                self.r_info & 0xff
            }
            #[cfg(target_pointer_width = "64")]
            {
                (self.r_info & 0xffff_ffff) as u32
            }
        }
    }

    const PT_DYNAMIC: u32 = 2;
    const PT_GNU_RELRO: u32 = 0x6474e552;
    const DT_RELA: u32 = 7;
    const DT_RELASZ: u32 = 8;
    const DT_RELAENT: u32 = 9;
    #[cfg(target_arch = "x86_64")]
    const R_RELATIVE: u32 = 8; // `R_X86_64_RELATIVE`
    #[cfg(target_arch = "x86")]
    const R_RELATIVE: u32 = 8; // `R_386_RELATIVE`
    #[cfg(target_arch = "aarch64")]
    const R_RELATIVE: u32 = 1027; // `R_AARCH64_RELATIVE`
    #[cfg(target_arch = "riscv64")]
    const R_RELATIVE: u32 = 3; // `R_RISCV_RELATIVE`
    #[cfg(target_arch = "arm")]
    const R_RELATIVE: u32 = 23; // `R_ARM_RELATIVE`
}

//! Program relocation.
//!
//! If we're a static PIE binary, we don't have a dynamic linker to perform
//! dynamic (startup-time) relocations for us, so we have to do it ourselves.
//! This is awkward, because it means we're running Rust code at a time when
//! the invariants that Rust code typically assume are not established yet.
//!
//! We use special functions implemented using `asm` to perform the actual
//! relocation memory accesses, so that Rust semantics don't have to be aware
//! of them.
//!
//! However, we need to be careful to avoid making any calls to other crates.
//! This includes functions in `core` that aren't implemented by the compiler,
//! so we can do `ptr == null()` but we can't do `ptr.is_null()` because `null`
//! is implemented by the compiler while `is_null` is not. And, it includes
//! code patterns that the compiler might implement as calls to functions like
//! `memset`.
//!
//! And we have to avoid reading any static variables that contain addresses,
//! including any tables the compiler might implicitly emit.
//!
//! Implicit calls to `memset` etc. could possibly be prevented by using
//! `#![no_builtins]`, however wouldn't fix the other problems, so we'd still
//! need to rely on testing the code to make sure it doesn't segfault in any
//! case. And, `#![no_builtins]` interferes with LTO, so we don't use it.

use crate::arch::{relocation_load, relocation_mprotect_readonly, relocation_store};
use core::ffi::c_void;
use core::ptr::{from_exposed_addr, null, null_mut};
use linux_raw_sys::elf::*;
use linux_raw_sys::general::{AT_ENTRY, AT_NULL, AT_PAGESZ, AT_PHDR, AT_PHENT, AT_PHNUM};

/// Locate the dynamic (startup-time) relocations and perform them.
///
/// This is unsafer than unsafe. It's meant to be called at a time when Rust
/// code is running but before relocations have been performed. There are no
/// guarantees that this code won't segfault at any moment.
///
/// In practice, things work if we don't make any calls to functions outside
/// of this crate, not counting functions directly implemented by the compiler.
/// So we can do eg. `x == null()` but we can't do `x.is_null()`, because
/// `null` is directly implemented by the compiler, while `is_null` is not.
///
/// We do all the relocation memory accesses using `asm`, as they happen
/// outside the Rust memory model. They read and write and `mprotect` memory
/// that Rust wouldn't think could be accessed.
///
/// And if this code panics, the panic code will probably segfault, because
/// `core::fmt` is known to use an address that needs relocation.
///
/// So yes, there's a reason this code is behind a feature flag.
#[cold]
pub(super) unsafe fn relocate(envp: *mut *mut u8) {
    // Locate the AUX records we need.
    let auxp = compute_auxp(envp);

    // The page size, segment headers info, and runtime entry address.
    let mut auxv_page_size = 0;
    let mut auxv_phdr = null();
    let mut auxv_phent = 0;
    let mut auxv_phnum = 0;
    let mut auxv_entry = 0;

    // Look through the AUX records to find the segment headers, page size,
    // and runtime entry address.
    let mut current_aux = auxp;
    loop {
        let Elf_auxv_t { a_type, a_val } = *current_aux;
        current_aux = current_aux.add(1);

        match a_type as _ {
            AT_PAGESZ => auxv_page_size = a_val.addr(),
            AT_PHDR => auxv_phdr = a_val.cast::<Elf_Phdr>(),
            AT_PHNUM => auxv_phnum = a_val.addr(),
            AT_PHENT => auxv_phent = a_val.addr(),
            AT_ENTRY => auxv_entry = a_val.addr(),
            AT_NULL => break,
            _ => (),
        }
    }

    // Compute the static address of `_start`.
    let static_start = compute_static_start();

    // Our offset is the difference between the static address of `_start` and
    // the actual runtime address of `_start` as given to us as the entry
    // address in the AUX table.
    let offset = auxv_entry.wrapping_sub(static_start);

    // If we're loaded at our static address, or if we were run by a dynamic
    // linker that has already performed the relocations, then there's nothing
    // to do.
    if offset == 0 {
        return;
    }

    // The location and size of the Dynamic section.
    let mut dynamic = null();
    let mut dynamic_size = 0;

    // The location and size of the `relro` region.
    let mut relro = 0;
    let mut relro_size = 0;

    // Next, look through the static segment headers (phdrs) to find the
    // Dynamic section and the relro description if present. In the Dynamic
    // section, find the rela and/or rel tables.
    let mut current_phdr = auxv_phdr;
    let phdrs_end = current_phdr
        .cast::<u8>()
        .add(auxv_phnum * auxv_phent)
        .cast();
    while current_phdr != phdrs_end {
        let phdr = &*current_phdr;
        current_phdr = current_phdr.cast::<u8>().add(auxv_phent).cast();

        match phdr.p_type {
            PT_DYNAMIC => {
                // We found the dynamic section. We model this as
                // `from_exposed_addr` for now because `with_addr` is a call.
                dynamic = from_exposed_addr(phdr.p_vaddr.wrapping_add(offset));
                dynamic_size = phdr.p_memsz;
            }
            PT_GNU_RELRO => {
                // A relro description is present. Make a note of it so that we
                // can mark the memory readonly after relocations are done.
                relro = phdr.p_vaddr;
                relro_size = phdr.p_memsz;
            }
            _ => (),
        }
    }

    // Rela tables contain `Elf_Rela` elements which have an
    // `r_addend` field.
    let mut rela_ptr: *const Elf_Rela = null();
    let mut rela_total_size = 0;
    let mut rela_entry_size = 0;

    // Rel tables contain `Elf_Rel` elements which lack an
    // `r_addend` field and store the addend in the memory to be
    // relocated.
    let mut rel_ptr: *const Elf_Rel = null();
    let mut rel_total_size = 0;
    let mut rel_entry_size = 0;

    // Look through the `Elf_Dyn` entries to find the location and
    // size of the relocation table(s).
    let mut current_dyn: *const Elf_Dyn = dynamic;
    let dyns_end = current_dyn.cast::<u8>().add(dynamic_size).cast();
    while current_dyn != dyns_end {
        let Elf_Dyn { d_tag, d_un } = &*current_dyn;
        current_dyn = current_dyn.add(1);

        match *d_tag {
            // We found a rela table. As above, model this as
            // `from_exposed_addr`.
            DT_RELA => rela_ptr = from_exposed_addr(d_un.d_ptr.wrapping_add(offset)),
            DT_RELASZ => rela_total_size = d_un.d_val as usize,
            DT_RELAENT => rela_entry_size = d_un.d_val as usize,

            // We found a rel table. As above, model this as
            // `from_exposed_addr`.
            DT_REL => rel_ptr = from_exposed_addr(d_un.d_ptr.wrapping_add(offset)),
            DT_RELSZ => rel_total_size = d_un.d_val as usize,
            DT_RELENT => rel_entry_size = d_un.d_val as usize,

            _ => (),
        }
    }

    // Perform the rela relocations.
    let mut current_rela = rela_ptr;
    let rela_end = current_rela.cast::<u8>().add(rela_total_size).cast();
    while current_rela != rela_end {
        let rela = &*current_rela;
        current_rela = current_rela.cast::<u8>().add(rela_entry_size).cast();

        // Calculate the location the relocation will apply at.
        let reloc_addr = rela.r_offset.wrapping_add(offset);

        // Perform the rela relocation.
        match rela.type_() {
            R_RELATIVE => {
                let addend = rela.r_addend;
                let reloc_value = addend.wrapping_add(offset);
                relocation_store(reloc_addr, reloc_value);
            }
            _ => unimplemented!(),
        }
    }

    // Perform the rel relocations.
    let mut current_rel = rel_ptr;
    let rel_end = current_rel.cast::<u8>().add(rel_total_size).cast();
    while current_rel != rel_end {
        let rel = &*current_rel;
        current_rel = current_rel.cast::<u8>().add(rel_entry_size).cast();

        // Calculate the location the relocation will apply at.
        let reloc_addr = rel.r_offset.wrapping_add(offset);

        // Perform the rel relocation.
        match rel.type_() {
            R_RELATIVE => {
                let addend = relocation_load(reloc_addr);
                let reloc_value = addend.wrapping_add(offset);
                relocation_store(reloc_addr, reloc_value);
            }
            _ => unimplemented!(),
        }
    }

    #[cfg(debug_assertions)]
    {
        // Check that the page size is a power of two.
        assert_eq!(auxv_page_size.count_ones(), 1);

        // This code doesn't rely on the offset being page aligned, but it is
        // useful to check to make sure we computed it correctly.
        assert_eq!(offset & (auxv_page_size - 1), 0);

        // Check that relocation did its job. Do the same static start
        // computation we did earlier; this time it should match the dynamic
        // address.
        assert_eq!(compute_static_start(), auxv_entry);
    }

    // If we saw a relro description, mark the memory readonly.
    if relro_size != 0 {
        let mprotect_addr = relro.wrapping_add(offset) & auxv_page_size.wrapping_neg();
        relocation_mprotect_readonly(mprotect_addr, relro_size);
    }
}

/// Compute the address of the AUX table.
unsafe fn compute_auxp(envp: *mut *mut u8) -> *const Elf_auxv_t {
    // Locate the AUX records we need. We don't use rustix to do this because
    // that would involve calling a function in another crate.
    let mut auxp = envp;
    // Don't use `is_null` because that's a call.
    while (*auxp) != null_mut() {
        auxp = auxp.add(1);
    }
    auxp.add(1).cast()
}

/// Compute the static address of `_start`.
///
/// This returns a `usize` because we don't dereference the address; we just
/// use it for address computation.
fn compute_static_start() -> usize {
    // This relies on the sneaky fact that we initialize a static variable with
    // the address of `_start`, and if we haven't performed relocations yet,
    // we'll be able to see the static address. Also, the program *just*
    // started so there are no other threads yet, so loading from static memory
    // without synchronization is fine. But we still use `asm` to do everything
    // we can to protect this code because the optimizer won't have any idea
    // what we're up to.
    struct StaticStart(*const c_void);
    unsafe impl Sync for StaticStart {}
    static STATIC_START: StaticStart = StaticStart(crate::arch::_start as *const c_void);
    let static_start_addr: *const *const c_void = &STATIC_START.0;
    unsafe { relocation_load(static_start_addr.addr()) }
}

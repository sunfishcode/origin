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

#![allow(clippy::cmp_null)]

use crate::arch::{
    dynamic_table_addr, ehdr_addr, relocation_load, relocation_mprotect_readonly, relocation_store,
};
use core::ffi::c_void;
use core::mem;
use core::ptr::{null, null_mut, with_exposed_provenance};
use linux_raw_sys::elf::*;
use linux_raw_sys::general::{AT_BASE, AT_ENTRY, AT_NULL, AT_PAGESZ};

// The Linux UAPI headers don't define the .relr types and consts yet.
#[allow(non_camel_case_types)]
type Elf_Relr = usize;

const DT_RELRSZ: usize = 35;
const DT_RELR: usize = 36;
#[cfg(debug_assertions)]
const DT_RELRENT: usize = 37;

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
    let mut auxv_base = 0;
    let mut auxv_page_size = 0;
    let mut auxv_entry = 0;

    // Look through the AUX records to find the segment headers, page size,
    // and runtime entry address.
    let mut current_aux = auxp;
    loop {
        let Elf_auxv_t { a_type, a_val } = *current_aux;
        current_aux = current_aux.add(1);

        match a_type as _ {
            AT_BASE => auxv_base = a_val.addr(),
            AT_PAGESZ => auxv_page_size = a_val.addr(),
            AT_ENTRY => auxv_entry = a_val.addr(),
            AT_NULL => break,
            _ => (),
        }
    }

    // There are four cases to consider here:
    //
    // 1. Static executable.
    //    This is the trivial case. No relocations necessary.
    // 2. Static pie executable.
    //    We need to relocate ourself. `AT_PHDR` points to our own program
    //    headers and `AT_ENTRY` to our own entry point.
    // 3. Dynamic pie executable with external dynamic linker.
    //    We don't need to relocate ourself as the dynamic linker has already
    //    done this. `AT_PHDR` points to our own program headers and `AT_ENTRY`
    //    to our own entry point. `AT_BASE` contains the relocation offset of
    //    the dynamic linker.
    // 4. Shared library acting as dynamic linker.
    //    We do need to relocate ourself. `AT_PHDR` doesn't point to our own
    //    program headers and `AT_ENTRY` doesn't point to our own entry point.
    //    `AT_BASE` contains our own relocation offset.

    if load_static_start() == auxv_entry {
        // This is case 1) or case 3). If `AT_BASE` doesn't exist, then we are
        // already loaded at our static address despite the lack of any dynamic
        // linker. As such it would be case 1). If `AT_BASE` does exist, we have
        // already been relocated by the dynamic linker, which is case 3).
        // In either case there is no need to do any relocations.
        return;
    }

    let offset = if auxv_base == 0 {
        // Obtain the static address of `_start`, which is recorded in the
        // entry field of the ELF header.
        let static_start = (*ehdr_addr()).e_entry;

        // This is case 2) as without dynamic linker `AT_BASE` doesn't exist
        // and we have already excluded case 1) above.
        auxv_entry.wrapping_sub(static_start)
    } else {
        // This is case 4) as `AT_BASE` indicates that there is a dynamic
        // linker, yet we are not already relocated by a dynamic linker.
        // `AT_BASE` contains the relocation offset of the dynamic linker.
        auxv_base
    };

    // This is case 2) or 4). We need to do all `R_RELATIVE` relocations.
    // There should be no other kind of relocation because we are either a
    // static PIE binary or a dynamic linker compiled with `-Bsymbolic`.

    // Compute the dynamic address of `_DYNAMIC`.
    let dynv = dynamic_table_addr();

    // Rela tables contain `Elf_Rela` elements which have an
    // `r_addend` field.
    let mut rela_ptr: *const Elf_Rela = null();
    let mut rela_total_size = 0;

    // Rel tables contain `Elf_Rel` elements which lack an
    // `r_addend` field and store the addend in the memory to be
    // relocated.
    let mut rel_ptr: *const Elf_Rel = null();
    let mut rel_total_size = 0;

    // Relr tables contain `Elf_Relr` elements which are a compact
    // way of representing relative relocations.
    let mut relr_ptr: *const Elf_Relr = null();
    let mut relr_total_size = 0;

    // Look through the `Elf_Dyn` entries to find the location and
    // size of the relocation table(s).
    let mut current_dyn: *const Elf_Dyn = dynv;
    loop {
        let Elf_Dyn { d_tag, d_un } = &*current_dyn;
        current_dyn = current_dyn.add(1);

        match *d_tag {
            // We found a rela table. As above, model this as
            // `with_exposed_provenance`.
            DT_RELA => rela_ptr = with_exposed_provenance(d_un.d_ptr.wrapping_add(offset)),
            DT_RELASZ => rela_total_size = d_un.d_val as usize,
            #[cfg(debug_assertions)]
            DT_RELAENT => debug_assert_eq!(d_un.d_val as usize, size_of::<Elf_Rela>()),

            // We found a rel table. As above, model this as
            // `with_exposed_provenance`.
            DT_REL => rel_ptr = with_exposed_provenance(d_un.d_ptr.wrapping_add(offset)),
            DT_RELSZ => rel_total_size = d_un.d_val as usize,
            #[cfg(debug_assertions)]
            DT_RELENT => debug_assert_eq!(d_un.d_val as usize, size_of::<Elf_Rel>()),

            // We found a relr table. As above, model this as
            // `with_exposed_provenance`.
            DT_RELR => relr_ptr = with_exposed_provenance(d_un.d_ptr.wrapping_add(offset)),
            DT_RELRSZ => relr_total_size = d_un.d_val as usize,
            #[cfg(debug_assertions)]
            DT_RELRENT => debug_assert_eq!(d_un.d_val as usize, size_of::<Elf_Relr>()),

            // End of the Dynamic section
            DT_NULL => break,

            _ => (),
        }
    }

    // Perform the rela relocations.
    let mut current_rela = rela_ptr;
    let rela_end = current_rela.byte_add(rela_total_size);
    while current_rela != rela_end {
        let rela = &*current_rela;
        current_rela = current_rela.add(1);

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
    let rel_end = current_rel.byte_add(rel_total_size);
    while current_rel != rel_end {
        let rel = &*current_rel;
        current_rel = current_rel.add(1);

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

    // Perform the relr relocations.
    let mut current_relr = relr_ptr;
    let relr_end = current_relr.byte_add(relr_total_size);
    let mut reloc_addr = 0;
    while current_relr != relr_end {
        let mut entry = *current_relr;
        current_relr = current_relr.add(1);

        if entry & 1 == 0 {
            // Entry encodes offset to relocate; calculate the location
            // the relocation will apply at.
            reloc_addr = offset + entry;

            // Perform the relr relocation.
            let addend = relocation_load(reloc_addr);
            let reloc_value = addend.wrapping_add(offset);
            relocation_store(reloc_addr, reloc_value);

            // Advance relocation location.
            reloc_addr += mem::size_of::<usize>();
        } else {
            // Entry encodes bitmask with locations to relocate; apply each entry.

            let mut i = 0;
            loop {
                // Shift to next item in the bitmask.
                entry >>= 1;
                if entry == 0 {
                    // No more entries left in the bitmask; early exit.
                    break;
                }

                if entry & 1 != 0 {
                    // Perform the relr relocation.
                    let addend = relocation_load(reloc_addr + i * mem::size_of::<usize>());
                    let reloc_value = addend.wrapping_add(offset);
                    relocation_store(reloc_addr + i * mem::size_of::<usize>(), reloc_value);
                }

                i += 1;
            }

            // Advance relocation location.
            reloc_addr += (mem::size_of::<usize>() * 8 - 1) * mem::size_of::<usize>();
        }
    }

    // FIXME split function into two here with a hint::black_box around the
    // function pointer to prevent the compiler from moving code between the
    // functions.

    #[cfg(debug_assertions)]
    {
        // Check that the page size is a power of two.
        assert!(auxv_page_size.is_power_of_two());

        // This code doesn't rely on the offset being page aligned, but it is
        // useful to check to make sure we computed it correctly.
        assert_eq!(offset & (auxv_page_size - 1), 0);

        // Check that relocation did its job. Do the same static start
        // computation we did earlier; this time it should match the dynamic
        // address.
        // AT_ENTRY points to the main executable's entry point rather than our
        // entry point when AT_BASE is not zero and thus a dynamic linker is in
        // use. In this case the assertion would fail.
        if auxv_base == 0 {
            assert_eq!(load_static_start(), auxv_entry);
        }
    }

    // Finally, look through the static segment headers (phdrs) to find the
    // the relro description if present. Also do a debug assertion that
    // the dynv argument matches the PT_DYNAMIC segment.
    extern "C" {
        #[link_name = "__ehdr_start"]
        static EHDR: Elf_Ehdr;
    }

    // The location and size of the `relro` region.
    let mut relro = 0;
    let mut relro_size = 0;

    let phentsize = EHDR.e_phentsize as usize;
    let mut current_phdr = with_exposed_provenance::<Elf_Phdr>(offset + EHDR.e_phoff);
    let phdrs_end = current_phdr.byte_add(EHDR.e_phnum as usize * phentsize);
    while current_phdr != phdrs_end {
        let phdr = &*current_phdr;
        current_phdr = current_phdr.byte_add(phentsize);

        match phdr.p_type {
            #[cfg(debug_assertions)]
            PT_DYNAMIC => {
                assert_eq!(dynv.addr(), phdr.p_vaddr.wrapping_add(offset));
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

/// Load the address of `_start` from static memory.
///
/// This function contains a static variable initialized with the address of
/// the `_start` function. This requires an `R_RELATIVE` relocation, because
/// it requires an absolute address exist in memory, rather than being able
/// to use PC-relative addressing. This allows us to tell whether relocation
/// has already been done or whether we need to do it.
///
/// This returns a `usize` because we don't dereference the address; we just
/// use it for address computation.
fn load_static_start() -> usize {
    // Initialize a static variable with the address of `_start`.
    struct StaticStart(*const c_void);
    unsafe impl Sync for StaticStart {}
    static STATIC_START: StaticStart = StaticStart(crate::arch::_start as *const c_void);

    // Use `relocation_load` to do the load because the optimizer won't have
    // any idea what we're up to.
    let static_start_addr: *const *const c_void = &STATIC_START.0;
    unsafe { relocation_load(static_start_addr.addr()) }
}

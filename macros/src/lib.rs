use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let main_fn: ItemFn = parse_macro_input!(input);
    let main_fn_ident = &main_fn.sig.ident;
    let asm_impl = asm_impl();

    quote! {
        #main_fn

        #[naked]
        #[no_mangle]
        unsafe extern "C" fn _start() -> ! {
            unsafe fn entry(mem: *mut usize) -> ! {
                let (argc, argv, envp) = origin::program::compute_args(mem);
                origin::program::init_runtime(mem, envp);
                origin::program::exit(#main_fn_ident())
            }

            #asm_impl
        }
    }
    .into()
}

/// Provides the asm implementation to start the program
fn asm_impl() -> TokenStream2 {
    quote! {
        // Jump to `entry`, passing it the initial stack pointer value as an
        // argument, a null return address, a null frame pointer, and an aligned
        // stack pointer. On many architectures, the incoming frame pointer is
        // already null.

        #[cfg(target_arch = "x86_64")]
        core::arch::asm!(
            "mov rdi, rsp", // Pass the incoming `rsp` as the arg to `entry`.
            "push rbp",     // Set the return address to zero.
            "jmp {entry}",  // Jump to `entry`.
            entry = sym entry,
            options(noreturn),
        );

        #[cfg(target_arch = "aarch64")]
        core::arch::asm!(
            "mov x0, sp",   // Pass the incoming `sp` as the arg to `entry`.
            "mov x30, xzr", // Set the return address to zero.
            "b {entry}",    // Jump to `entry`.
            entry = sym entry,
            options(noreturn),
        );

        #[cfg(target_arch = "arm")]
        core::arch::asm!(
            "mov r0, sp\n", // Pass the incoming `sp` as the arg to `entry`.
            "mov lr, #0",   // Set the return address to zero.
            "b {entry}",    // Jump to `entry`.
            entry = sym entry,
            options(noreturn),
        );

        #[cfg(target_arch = "riscv64")]
        core::arch::asm!(
            "mv a0, sp",    // Pass the incoming `sp` as the arg to `entry`.
            "mv ra, zero",  // Set the return address to zero.
            "mv fp, zero",  // Set the frame address to zero.
            "tail {entry}", // Jump to `entry`.
            entry = sym entry,
            options(noreturn),
        );

        #[cfg(target_arch = "x86")]
        core::arch::asm!(
            "mov eax, esp", // Save the incoming `esp` value.
            "push ebp",     // Pad for alignment.
            "push ebp",     // Pad for alignment.
            "push ebp",     // Pad for alignment.
            "push eax",     // Pass saved the incoming `esp` as the arg to `entry`.
            "push ebp",     // Set the return address to zero.
            "jmp {entry}",  // Jump to `entry`.
            entry = sym entry,
            options(noreturn),
        );
    }
}

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let main_fn: ItemFn = parse_macro_input!(input);

    quote! {
        #main_fn

        #[naked]
        #[no_mangle]
        unsafe extern "C" fn _start() -> ! {
            unsafe fn entry(mem: *mut usize) -> ! {
                let (argc, argv, envp) = origin::program::compute_args(mem);
                origin::program::init_runtime(mem, envp);
                origin::program::exit(main())
            }

            #[cfg(target_arch = "x86_64")]
            core::arch::asm!(
                "mov rdi, rsp", // Pass the incoming `rsp` as the arg to `entry`.
                "push rbp",     // Set the return address to zero.
                "jmp {entry}",  // Jump to `entry`.
                entry = sym entry,
                options(noreturn),
            );
        }
    }
    .into()
}

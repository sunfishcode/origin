//! Limited wrapper around and polyfill for the `#[naked]` attribute.
//!
//! # Example
//!
//! ```no_compile
//! naked!(
//!     "
//!     A documentation comment.
//!
//!     In the macro expansion, this string will be expanded as a documentation
//!     comment for the generated code.
//!     ";
//!
//!     // Declare a `pub(crate` function named `function_name` with no
//!     // arguments that does not return.
//!     pub(crate) fn function_name() -> !;
//!
//!     // Assembly code for the body.
//!     "assembly code here",
//!     "more assembly code here",
//!     "we can even use {symbols} like {this}",
//!
//!     // Provide symbols for use in the assembly code.
//!     symbols = sym path::to::symbols,
//!     this = sym path::to::this;
//!
//!     // Add additional `asm` options.
//!     options(noreturn)
//! );
//! ```

#![allow(unused_macros)]

/// `#[naked]` is nightly-only. We use it when we can, and fall back to
/// `global_asm` otherwise. This macro supports a limited subset of the
/// features of `#[naked]`.
#[cfg(feature = "nightly")]
macro_rules! naked {
    (
        $doc:literal;
        $vis:vis fn $name:ident $args:tt -> $ret:ty;
        $($code:literal),*;
        $($label:ident = $kind:ident $path:path),*;
        options($($option:ident),*)
    ) => {
        #[doc = $doc]
        #[naked]
        #[no_mangle]
        $vis unsafe extern "C" fn $name $args -> $ret {
            asm!(
                $($code),*,
                $($label = $kind $path,)*
                options($($option),*),
            )
        }
    };
}

/// `#[naked]` is nightly-only. We use it when we can, and fall back to
/// `global_asm` otherwise. This macro supports a limited subset of the
/// features of `#[naked]`.
#[cfg(not(feature = "nightly"))]
macro_rules! naked {
    (
        $doc:literal;
        $vis:vis fn $name:ident $args:tt -> $ret:ty;
        $($code:literal),*;
        $($label:ident = $kind:ident $path:path),*;
        options($($option:ident),*)
    ) => {
        extern "C" {
            #[doc = $doc]
            $vis fn $name $args -> $ret;
        }
        core::arch::global_asm!(
            concat!(".global ", stringify!($name)),
            concat!(".type ", stringify!($name), ", @function"),
            concat!(stringify!($name), ":"),
            $($code),*,
            concat!(".size ", stringify!($name), ", .-", stringify!($name)),
            $($label = $kind $path),*
            // Omit the options because this is a `global_asm`.
        );
    };
}

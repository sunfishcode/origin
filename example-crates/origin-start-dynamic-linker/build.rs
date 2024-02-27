fn main() {
    // Pass -nostartfiles to the linker. In the future this could be obviated
    // by a `no_entry` feature: <https://github.com/rust-lang/rfcs/pull/2735>
    println!("cargo:rustc-link-arg=-nostartfiles");
    // Set entry point for the dynamic library, making it capable of being
    // executed like an executable and being used as dynamic linker.
    println!("cargo:rustc-link-arg=-Wl,-e,_start");
    // Prevent non R_RELATIVE relocations by forcing all references to global
    // symbols to be resolved locally.
    println!("cargo:rustc-link-arg=-Wl,-Bsymbolic");
}

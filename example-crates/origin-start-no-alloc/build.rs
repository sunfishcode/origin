fn main() {
    // Pass -nostartfiles to the linker. In the future this could be obviated
    // by a `no_entry` feature: <https://github.com/rust-lang/rfcs/pull/2735>
    println!("cargo:rustc-link-arg=-nostartfiles");
}

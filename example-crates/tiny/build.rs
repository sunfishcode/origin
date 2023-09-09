fn main() {
    // Pass -nostartfiles to the linker. In the future this could be obviated
    // by a `no_entry` feature: <https://github.com/rust-lang/rfcs/pull/2735>
    println!("cargo:rustc-link-arg=-nostartfiles");

    // The following options optimize for code size!

    // Tell the linker to exclude the .eh_frame_hdr section.
    println!("cargo:rustc-link-arg=-Wl,--no-eh-frame-hdr");
    // Tell the linker not to page-align sections.
    println!("cargo:rustc-link-arg=-Wl,-n");
    // Tell the linker to make the text and data readable and writeable. This
    // allows them to occupy the same page.
    println!("cargo:rustc-link-arg=-Wl,-N");
    // Tell the linker to exclude the `.note.gnu.build-id` section.
    println!("cargo:rustc-link-arg=-Wl,--build-id=none");
    // Disable PIE, which adds some code size.
    println!("cargo:rustc-link-arg=-Wl,--no-pie");
    // Disable the `GNU-stack` segment, if we're using lld.
    println!("cargo:rustc-link-arg=-Wl,-z,nognustack");
}

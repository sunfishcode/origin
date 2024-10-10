//! Run the programs in the `test-crates` directory and compare their
//! outputs with expected outputs.

// Most of the test crates only work with nightly.
#![cfg(feature = "nightly")]

mod utils;

fn test_crate(
    name: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    stdout: &'static str,
    stderr: &'static str,
    code: Option<i32>,
) {
    utils::test_crate("test", "run", name, args, envs, stdout, stderr, code);
}

#[test]
fn test_tls() {
    test_crate(
        "origin-start",
        &["--bin=tls", "--release"],
        &[],
        "",
        "",
        Some(200),
    );
}

#[test]
fn test_tls_crt_static() {
    test_crate(
        "origin-start",
        &["--bin=tls", "--features=origin/experimental-relocate"],
        &[("RUSTFLAGS", "-C target-feature=+crt-static")],
        "",
        "",
        Some(200),
    );
}

#[test]
fn test_tls_crt_static_relr() {
    test_crate(
        "origin-start",
        &["--bin=tls", "--features=origin/experimental-relocate"],
        // This works with ld.bfd. ld.lld uses --pack-dyn-relocs=relr instead.
        // FIXME detect which linker is used and pick the appropriate flag.
        &[(
            "RUSTFLAGS",
            "-C target-feature=+crt-static -C link-arg=-Wl,-z,pack-relative-relocs",
        )],
        "",
        "",
        Some(200),
    );
}

#[test]
fn test_tls_crt_static_relocation_static() {
    test_crate(
        "origin-start",
        &["--bin=tls"],
        &[(
            "RUSTFLAGS",
            "-C target-feature=+crt-static -C relocation-model=static",
        )],
        "",
        "",
        Some(200),
    );
}

#[test]
fn test_thread_id() {
    test_crate("origin-start", &["--bin=thread-id"], &[], "", "", Some(201));
}

#[test]
fn test_detach() {
    test_crate("origin-start", &["--bin=detach"], &[], "", "", Some(202));
}

#[test]
fn test_canary() {
    test_crate("origin-start", &["--bin=canary"], &[], "", "", Some(203));
}

#[test]
fn test_program_dtors_adding_dtors() {
    test_crate(
        "origin-start",
        &["--bin=program-dtors-adding-dtors"],
        &[],
        "",
        "",
        Some(0),
    );
}

#[test]
fn test_thread_dtors_adding_dtors() {
    test_crate(
        "origin-start",
        &["--bin=thread-dtors-adding-dtors"],
        &[],
        "",
        "",
        Some(0),
    );
}

#[test]
fn test_main_thread_dtors_adding_dtors() {
    test_crate(
        "origin-start",
        &["--bin=main-thread-dtors-adding-dtors"],
        &[],
        "",
        "",
        Some(0),
    );
}

//! Run the programs in the `example-crates` directory and compare their
//! outputs with expected outputs.

#![feature(cfg_target_abi)]

use similar_asserts::assert_eq;

macro_rules! assert_eq_str {
    ($a:expr, $b:expr) => {{
        assert_eq!(String::from_utf8_lossy($a).lines().collect::<Vec<_>>(), String::from_utf8_lossy($b).lines().collect::<Vec<_>>());
        assert_eq!($a, $b);
    }};

    ($a:expr, $b:expr, $($arg:tt)+) => {{
        assert_eq!(String::from_utf8_lossy($a).lines().collect::<Vec<_>>(), String::from_utf8_lossy($b).lines().collect::<Vec<_>>(), $($arg)+);
        assert_eq!($a, $b, $($arg)+);
    }};
}

fn test_crate(name: &str, args: &[&str], envs: &[(&str, &str)], stdout: &str, stderr: &str) {
    use std::process::Command;

    let mut command = Command::new("cargo");
    command.arg("run").arg("--quiet");
    command.args(args);
    command.envs(envs.iter().cloned());
    command.current_dir(format!("example-crates/{}", name));
    dbg!(&command);
    let output = command.output().unwrap();

    assert_eq_str!(
        stderr.as_bytes(),
        &output.stderr,
        "example {} had unexpected stderr, with {:?}",
        name,
        output
    );

    assert_eq_str!(
        stdout.as_bytes(),
        &output.stdout,
        "example {} had unexpected stdout, with {:?}",
        name,
        output
    );

    assert!(
        output.status.success(),
        "example {} failed with {:?}",
        name,
        output
    );
}

/// Stderr output for most of the example crates.
const COMMON_STDERR: &'static str = "Hello from main thread\n\
    Hello from child thread\n\
    Hello from child thread's at_thread_exit handler\n\
    Goodbye from main\n\
    Hello from a main-thread at_thread_exit handler\n\
    Hello from an at_exit handler\n";

/// Stderr output for the origin-start-no-alloc crate.
const NO_ALLOC_STDERR: &'static str = "Hello!\n";

#[test]
fn example_crate_basic() {
    test_crate("basic", &[], &[], "", COMMON_STDERR);
}

/// Like `example_crate_basic` but redundantly call `relocate`.
#[test]
fn example_crate_basic_relocate() {
    test_crate(
        "basic",
        &["--features=origin/experimental-relocate"],
        &[],
        "",
        COMMON_STDERR,
    );
}

#[test]
fn example_crate_no_std() {
    test_crate("no-std", &[], &[], "", COMMON_STDERR);
}

/// Like `example_crate_no_std` but redundantly call `relocate`.
#[test]
fn example_crate_no_std_relocate() {
    test_crate(
        "no-std",
        &["--features=origin/experimental-relocate"],
        &[],
        "",
        COMMON_STDERR,
    );
}

#[test]
fn example_crate_external_start() {
    test_crate("external-start", &[], &[], "", COMMON_STDERR);
}

/// Like `example_crate_external_start` but redundantly call `relocate`.
#[test]
fn example_crate_external_start_relocate() {
    test_crate(
        "external-start",
        &["--features=origin/experimental-relocate"],
        &[],
        "",
        COMMON_STDERR,
    );
}

#[test]
fn example_crate_origin_start() {
    // Use a dynamic linker.
    test_crate("origin-start", &[], &[], "", COMMON_STDERR);
}

/// Use a dynamic linker, redundantly run `relocate`.
#[test]
fn example_crate_origin_start_relocate() {
    test_crate(
        "origin-start",
        &["--features=origin/experimental-relocate"],
        &[],
        "",
        COMMON_STDERR,
    );
}

/// Don't use a dynamic linker, run `relocate`.
#[test]
fn example_crate_origin_start_crt_static() {
    test_crate(
        "origin-start",
        &["--features=origin/experimental-relocate"],
        &[("RUSTFLAGS", "-C target-feature=+crt-static")],
        "",
        COMMON_STDERR,
    );
}

/// Don't use a dynamic linker, use static relocs, don't run `relocate`.
#[test]
fn example_crate_origin_start_crt_static_relocation_static() {
    test_crate(
        "origin-start",
        &[],
        &[(
            "RUSTFLAGS",
            "-C target-feature=+crt-static -C relocation-model=static",
        )],
        "",
        COMMON_STDERR,
    );
}

/// Don't use a dynamic linker, use static relocs, redundantly run `relocate`.
#[test]
fn example_crate_origin_start_crt_static_relocation_static_relocate() {
    test_crate(
        "origin-start",
        &["--features=origin/experimental-relocate"],
        &[(
            "RUSTFLAGS",
            "-C target-feature=+crt-static -C relocation-model=static",
        )],
        "",
        COMMON_STDERR,
    );
}

/// Use a dynamic linker.
#[test]
fn example_crate_origin_start_no_alloc() {
    test_crate("origin-start-no-alloc", &[], &[], "", NO_ALLOC_STDERR);
}

/// Use a dynamic linker, redundantly run `relocate`.
#[test]
fn example_crate_origin_start_no_alloc_relocate() {
    test_crate(
        "origin-start-no-alloc",
        &["--features=origin/experimental-relocate"],
        &[],
        "",
        NO_ALLOC_STDERR,
    );
}

/// Don't use a dynamic linker, run `relocate`.
#[test]
fn example_crate_origin_start_no_alloc_crt_static() {
    test_crate(
        "origin-start-no-alloc",
        &["--features=origin/experimental-relocate"],
        &[("RUSTFLAGS", "-C target-feature=+crt-static")],
        "",
        NO_ALLOC_STDERR,
    );
}

/// Don't use a dynamic linker, use static relocs, don't run `relocate`.
#[test]
fn example_crate_origin_start_no_alloc_crt_static_relocation_static() {
    test_crate(
        "origin-start-no-alloc",
        &[],
        &[(
            "RUSTFLAGS",
            "-C target-feature=+crt-static -C relocation-model=static",
        )],
        "",
        NO_ALLOC_STDERR,
    );
}

/// Don't use a dynamic linker, use static relocs, redundantly run `relocate`.
#[test]
fn example_crate_origin_start_no_alloc_crt_static_relocation_static_relocate() {
    test_crate(
        "origin-start-no-alloc",
        &["--features=origin/experimental-relocate"],
        &[(
            "RUSTFLAGS",
            "-C target-feature=+crt-static -C relocation-model=static",
        )],
        "",
        NO_ALLOC_STDERR,
    );
}

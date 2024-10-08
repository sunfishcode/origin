//! Run the programs in the `example-crates` directory and compare their
//! outputs with expected outputs.

// Most of the example crates only work with nightly.
#![cfg(feature = "nightly")]

mod utils;

use utils::{COMMON_STDERR, NO_ALLOC_STDERR};

fn test_crate(
    name: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    stdout: &'static str,
    stderr: &'static str,
    code: Option<i32>,
) {
    utils::test_crate("example", "run", name, args, envs, stdout, stderr, code);
}

#[test]
fn example_crate_basic() {
    test_crate("basic", &[], &[], "", COMMON_STDERR, None);
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
        None,
    );
}

#[test]
fn example_crate_no_std() {
    test_crate("no-std", &[], &[], "", COMMON_STDERR, None);
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
        None,
    );
}

#[test]
fn example_crate_external_start() {
    test_crate("external-start", &[], &[], "", COMMON_STDERR, None);
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
        None,
    );
}

#[test]
fn example_crate_origin_start() {
    // Use a dynamic linker.
    test_crate("origin-start", &[], &[], "", COMMON_STDERR, None);
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
        None,
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
        None,
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
        None,
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
        None,
    );
}

/// Act as dynamic linker, run `relocate`.
#[test]
fn example_crate_origin_start_dynamic_linker() {
    use assert_cmd::Command;

    utils::test_crate(
        "example",
        "build",
        "origin-start-dynamic-linker",
        &[],
        &[],
        "",
        "",
        None,
    );

    let arch = utils::arch();
    let target_dir = format!("example-crates/origin-start-dynamic-linker/target/{arch}/debug");
    let linker = if let Ok(linker) = std::env::var(format!(
        "CARGO_TARGET_{}_LINKER",
        arch.to_uppercase().replace('-', "_")
    )) {
        vec![format!("-Clinker={linker}")]
    } else {
        vec![]
    };

    // Build a dummy executable with the previously built liborigin_start.so as
    // dynamic linker.
    let assert = Command::new("rustc")
        .args(&[
            "-",
            "--crate-type",
            "bin",
            "--target",
            &arch,
            "-Cpanic=abort",
            "-Clink-arg=-nostartfiles",
        ])
        .args(linker)
        .arg(format!(
            "-Clink-arg=-Wl,--dynamic-linker={target_dir}/liborigin_start.so",
        ))
        .arg("-o")
        .arg(format!("{target_dir}/libempty.so"))
        .write_stdin(
            "#![no_std]\
            #![no_main]\
            #[panic_handler]\
            fn panic_handler(_: &core::panic::PanicInfo) -> ! { loop {} }",
        )
        .assert();
    assert.stdout("").stderr("").success();

    // Run the executable
    if let Ok(runner) = std::env::var(format!(
        "CARGO_TARGET_{}_RUNNER",
        arch.to_uppercase().replace('-', "_")
    )) {
        let assert = Command::new(runner.split(' ').next().unwrap())
            .arg(format!("{target_dir}/libempty.so"))
            .assert();
        assert.stdout("").stderr(COMMON_STDERR).success();
    } else {
        let assert = Command::new(format!("{target_dir}/libempty.so")).assert();
        assert.stdout("").stderr(COMMON_STDERR).success();
    };
}

/// Use a dynamic linker.
#[test]
fn example_crate_origin_start_no_alloc() {
    test_crate("origin-start-no-alloc", &[], &[], "", NO_ALLOC_STDERR, None);
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
        None,
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
        None,
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
        None,
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
        None,
    );
}

#[test]
fn example_crate_origin_start_lto() {
    // Use a dynamic linker.
    test_crate(
        "origin-start-lto",
        &["--release"],
        &[],
        "",
        COMMON_STDERR,
        None,
    );
}

/// Use a dynamic linker, redundantly run `relocate`.
#[test]
fn example_crate_origin_start_lto_relocate() {
    test_crate(
        "origin-start-lto",
        &["--release", "--features=origin/experimental-relocate"],
        &[],
        "",
        COMMON_STDERR,
        None,
    );
}

/// Don't use a dynamic linker, run `relocate`.
#[test]
fn example_crate_origin_start_lto_crt_static() {
    test_crate(
        "origin-start-lto",
        &["--release", "--features=origin/experimental-relocate"],
        &[("RUSTFLAGS", "-C target-feature=+crt-static")],
        "",
        COMMON_STDERR,
        None,
    );
}

/// Don't use a dynamic linker, use static relocs, don't run `relocate`.
#[test]
fn example_crate_origin_start_lto_crt_static_relocation_static() {
    test_crate(
        "origin-start-lto",
        &["--release"],
        &[(
            "RUSTFLAGS",
            "-C target-feature=+crt-static -C relocation-model=static",
        )],
        "",
        COMMON_STDERR,
        None,
    );
}

/// Don't use a dynamic linker, use static relocs, redundantly run `relocate`.
#[test]
fn example_crate_origin_start_lto_crt_static_relocation_static_relocate() {
    test_crate(
        "origin-start-lto",
        &["--release", "--features=origin/experimental-relocate"],
        &[(
            "RUSTFLAGS",
            "-C target-feature=+crt-static -C relocation-model=static",
        )],
        "",
        COMMON_STDERR,
        None,
    );
}

#[test]
fn example_crate_tiny() {
    test_crate("tiny", &["--release"], &[], "", "", Some(42));
}

//! Run the programs in the `example-crates` directory and compare their
//! outputs with expected outputs.

#![feature(cfg_target_abi)]

fn test_crate(
    name: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    stdout: &'static str,
    stderr: &'static str,
) {
    use assert_cmd::Command;

    let mut command = Command::new("cargo");
    command.arg("run").arg("--quiet");
    command.args(args);
    command.envs(envs.iter().cloned());
    command.current_dir(format!("example-crates/{}", name));
    command.assert().stdout(stdout).stderr(stderr).success();
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

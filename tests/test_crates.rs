//! Run the programs in the `test-crates` directory and compare their
//! outputs with expected outputs.

#![feature(cfg_target_abi)]

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

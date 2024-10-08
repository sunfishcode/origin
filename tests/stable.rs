//! Run programs in the `example-crates` directory that can be run with
//! stable Rust and compare their outputs with expected outputs.

mod utils;

use utils::COMMON_STDERR;

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

#[cfg(not(feature = "nightly"))]
#[test]
fn example_crate_basic() {
    test_crate("basic", &[], &[], "", COMMON_STDERR, None);
}

#[cfg(not(feature = "nightly"))]
#[test]
fn example_crate_origin_start_stable() {
    test_crate("origin-start-stable", &[], &[], "", COMMON_STDERR, None);
}

#[cfg(feature = "nightly")]
#[test]
fn example_crate_origin_start_stable_with_nightly() {
    test_crate(
        "origin-start-stable",
        &["--features=nightly"],
        &[],
        "",
        COMMON_STDERR,
        None,
    );
}

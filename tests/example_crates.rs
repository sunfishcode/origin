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

fn test_crate(name: &str, features: &str, stdout: &str, stderr: &str) {
    use std::process::Command;

    let mut command = Command::new("cargo");
    command.arg("run").arg("--quiet");
    if !features.is_empty() {
        command
            .arg("--no-default-features")
            .arg("--features")
            .arg(features);
    }
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

#[test]
fn example_crate_basic() {
    test_crate(
        "basic",
        "",
        "",
        "Hello from main thread\n\
    Hello from child thread\n\
    Hello from child thread's at_thread_exit handler\n\
    Goodbye from main\n\
    Hello from a main-thread at_thread_exit handler\n\
    Hello from an at_exit handler\n",
    );
}

#[test]
fn example_crate_no_std() {
    test_crate(
        "no-std",
        "",
        "",
        "Hello from main thread\n\
    Hello from child thread\n\
    Hello from child thread's at_thread_exit handler\n\
    Goodbye from main\n\
    Hello from a main-thread at_thread_exit handler\n\
    Hello from an at_exit handler\n",
    );
}

#[test]
fn example_crate_external_start() {
    test_crate(
        "external-start",
        "",
        "",
        "Hello from main thread\n\
    Hello from child thread\n\
    Hello from child thread's at_thread_exit handler\n\
    Goodbye from main\n\
    Hello from a main-thread at_thread_exit handler\n\
    Hello from an at_exit handler\n",
    );
}

#[test]
fn example_crate_origin_start() {
    test_crate(
        "origin-start",
        "",
        "",
        "Hello from main thread\n\
    Hello from child thread\n\
    Hello from child thread's at_thread_exit handler\n\
    Goodbye from main\n\
    Hello from a main-thread at_thread_exit handler\n\
    Hello from an at_exit handler\n",
    );
}

#[test]
fn example_crate_origin_start_no_alloc() {
    test_crate("origin-start-no-alloc", "", "", "Hello!\n");
}

//! Run the programs in the `test-crates` directory and compare their outputs
//! with expected outputs.

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

    #[cfg(target_arch = "x86_64")]
    let arch = "x86_64";
    #[cfg(target_arch = "aarch64")]
    let arch = "aarch64";
    #[cfg(target_arch = "riscv64")]
    let arch = "riscv64gc";
    #[cfg(target_arch = "x86")]
    let arch = "i686";
    #[cfg(target_arch = "arm")]
    let arch = "armv5te";
    #[cfg(target_env = "gnueabi")]
    let env = "gnueabi";
    #[cfg(all(target_env = "gnu", target_abi = "eabi"))]
    let env = "gnueabi";
    #[cfg(all(target_env = "gnu", not(target_abi = "eabi")))]
    let env = "gnu";

    let mut command = Command::new("cargo");
    command.arg("run").arg("--quiet");
    if !features.is_empty() {
        command
            .arg("--no-default-features")
            .arg("--features")
            .arg(features);
    }
    // Always pass a --target option even when we're not cross-compiling;
    // see `origin-start`'s README.md for more info.
    command
        .arg(&format!("--target={}-unknown-linux-{}", arch, env))
        .current_dir(format!("test-crates/{}", name));
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
fn test_crate_basic() {
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
fn test_crate_no_std() {
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
fn test_crate_external_start() {
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
fn test_crate_origin_start() {
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
fn test_crate_origin_start_no_alloc() {
    test_crate("origin-start-no-alloc", "", "", "Hello!\n");
}

#![allow(dead_code)]

use assert_cmd::Command;

pub fn arch() -> String {
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
    #[cfg(all(target_env = "gnu", target_abi = "eabi"))]
    let env = "gnueabi";
    #[cfg(all(target_env = "gnu", not(target_abi = "eabi")))]
    let env = "gnu";

    format!("{arch}-unknown-linux-{env}")
}

pub fn run_test(
    dir: &str,
    cmd: &str,
    name: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) -> std::process::Command {
    let mut command = std::process::Command::new("cargo");
    command.arg(cmd).arg("--quiet");
    command.arg(format!("--target={}", arch()));
    command.args(args);
    command.envs(envs.iter().copied());
    command.current_dir(format!("{dir}-crates/{name}"));
    command
}

pub fn test_crate(
    dir: &str,
    cmd: &str,
    name: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    stdout: &'static str,
    stderr: &'static str,
    code: Option<i32>,
) {
    let command = run_test(dir, cmd, name, args, envs);
    let mut command = Command::from_std(command);
    let assert = command.assert();

    let assert = assert.stdout(stdout).stderr(stderr);
    if let Some(code) = code {
        assert.code(code);
    } else {
        assert.success();
    }
}

/// Stderr output for most of the example crates.
pub const COMMON_STDERR: &str = "Hello from main thread\n\
    Hello from child thread\n\
    Hello from child thread's `thread::at_exit` handler\n\
    Goodbye from main\n\
    Hello from a main-thread `thread::at_exit` handler\n\
    Hello from a `program::at_exit` handler\n";

/// Stderr output for the origin-start-no-alloc crate.
pub const NO_ALLOC_STDERR: &str = "Hello!\n";

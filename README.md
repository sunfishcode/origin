<div align="center">
  <h1><code>origin</code></h1>

  <p>
    <strong>Program and thread startup and shutdown in Rust</strong>
  </p>

  <p>
    <a href="https://github.com/sunfishcode/origin/actions?query=workflow%3ACI"><img src="https://github.com/sunfishcode/origin/workflows/CI/badge.svg" alt="Github Actions CI Status" /></a>
    <a href="https://bytecodealliance.zulipchat.com/#narrow/stream/206238-general"><img src="https://img.shields.io/badge/zulip-join_chat-brightgreen.svg" alt="zulip chat" /></a>
    <a href="https://crates.io/crates/origin"><img src="https://img.shields.io/crates/v/origin.svg" alt="crates.io page" /></a>
    <a href="https://docs.rs/origin"><img src="https://docs.rs/origin/badge.svg" alt="docs.rs docs" /></a>
  </p>
</div>

Origin implements program startup and shutdown, as well as thread startup and
shutdown, for Linux, implemented in Rust.

Program startup and shutdown for Linux is traditionally implemented in crt1.o,
and the libc functions `exit`, `atexit`, and `_exit`. And thread startup and
shutdown are traditionally implemented in libpthread functions
`pthread_create`, `pthread_join`, `pthread_detach`, and so on. Origin provides
its own implementations of this functionality, written in Rust.

For a C-ABI-compatible interface to this functionality, see [c-scape].

This is used by the [Mustang] project in its libc implementation, and in the
[origin-studio] project in its std implementation, which are two different
ways to support building Rust programs written entirely in Rust.

Origin can also be used on its own, in several different configurations:

 - The [basic example] shows a simple example of using origin as a simple
   library. In this configuration, libc is doing most of the work.

 - The [no-std example] uses `no_std` and starts the program using Rust's
   `#[start]` feature, and then hands control to origin. libc is still
   doing most of the work here.

 - The [external-start example] uses `no_std` and `no_main`, and starts the
   program by taking over control from libc as soon as possible, and then
   hands control to origin. origin handles program and thread startup and
   shutdown once it takes control.

 - The [origin-start example] uses `no_std` and `no_main`, and lets origin
   start the program using its own program entrypoint. This version must be
   compiled with an explicit `--target=` flag, even if it's the same as the
   host, because it uses special RUSTFLAGS, and an explicit `--target` flag
   prevents those flags from being passed to build scripts. origin handles
   program and thread startup and shutdown and no part of libc is used.

[basic example]: https://github.com/sunfishcode/origin/blob/main/test-crates/basic/README.md
[no-std example]: https://github.com/sunfishcode/origin/blob/main/test-crates/no-std/README.md
[external-start example]: https://github.com/sunfishcode/origin/blob/main/test-crates/external-start/README.md
[Mustang]: https://github.com/sunfishcode/mustang/
[origin-studio]: https://github.com/sunfishcode/origin-studio
[origin-start example]: https://github.com/sunfishcode/origin/blob/main/test-crates/origin-start/README.md
[c-scape]: https://crates.io/crates/c-scape/

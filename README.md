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
   start the program using its own program entrypoint. origin handles program
   and thread startup and shutdown and no part of libc is used. This is the
   approach that [origin-studio] uses.

 - The [origin-start-no-alloc example] is like origin-start, but disables the
   "alloc" and "thread" features, so that it doesn't need to pull in a global
   allocator.

## Fully static linking

The resulting executables in the origin-start and origin-start-no-alloc
examples don't depend on any dynamic libraries, however by default they do
still depend on a dynamic linker.

Origin doesn't currently support Position-Independent Executables (PIE), so
linking with just `RUSTFLAGS=-C target-feature=+crt-static` produces executable
which don't work. A workaround is to disable PIE, using
`RUSTFLAGS=-C target-feature=+crt-static -C relocation-model=static`. This
produces a fully static executable.

[basic example]: https://github.com/sunfishcode/origin/blob/main/example-crates/basic/README.md
[no-std example]: https://github.com/sunfishcode/origin/blob/main/example-crates/no-std/README.md
[external-start example]: https://github.com/sunfishcode/origin/blob/main/example-crates/external-start/README.md
[origin-start example]: https://github.com/sunfishcode/origin/blob/main/example-crates/origin-start/README.md
[origin-start-no-alloc example]: https://github.com/sunfishcode/origin/blob/main/example-crates/origin-start-no-alloc/README.md
[Mustang]: https://github.com/sunfishcode/mustang/
[origin-studio]: https://github.com/sunfishcode/origin-studio
[c-scape]: https://crates.io/crates/c-scape/

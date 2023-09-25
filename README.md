<div align="center">
  <h1>Origin</h1>

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

This is used by [Mustang] and [Eyra] in their libc implementations, and in
the [Origin Studio] project in its std implementation, which are three different
ways to support building Rust programs written entirely in Rust.

## Example crates

Origin can also be used on its own, in several different configurations:

 - The [basic example] shows a simple example of using Origin as a simple
   library. In this configuration, libc is doing most of the work.

 - The [no-std example] uses `no_std` and starts the program using Rust's
   `#[start]` feature, and then hands control to Origin. libc is still
   doing most of the work here.

 - The [external-start example] uses `no_std` and `no_main`, and starts the
   program by taking over control from libc as soon as possible, and then
   hands control to Origin. Origin handles program and thread startup and
   shutdown once it takes control.

 - The [origin-start example] uses `no_std` and `no_main`, and lets Origin
   start the program using its own program entrypoint. Origin handles program
   and thread startup and shutdown and no part of libc is used. This is the
   approach that [Origin Studio] uses.

 - The [origin-start-no-alloc example] is like origin-start, but disables the
   "alloc" and "thread" features, since Origin's "thread" feature currently
   depends on "alloc". Without "alloc", functions that return owned strings
   or `Vec`s are not available. In this mode, Origin avoids using a
   global allocator entirely.

 - The [origin-start-lto example] is like origin-start, but builds with LTO.

 - The [tiny example] is like origin-start, but builds with optimization flags,
   disables features, and adds an objcopy trick to produce a very small
   binary—408 bytes on x86-64!

## Fully static linking

The resulting executables in the origin-start, origin-start-no-alloc, and
origin-start-lto examples don't depend on any dynamic libraries, however by
default they do still depend on a dynamic linker.

For fully static linking, there are two options:

 - Build with `RUSTFLAGS=-C target-feature=+crt-static -C relocation-model=static`.
   This disables PIE mode, which is safer in terms of Origin's code, but loses
   the security benefits of Address-Space Layout Randomization (ASLR).

 - Build with `RUSTFLAGS=-C target-feature=+crt-static` and enable
   Origin's `experimental-relocate` feature. This allows PIE mode to work,
   however it does so by enabling some highly experimental code in origin for
   performing relocations.

[basic example]: https://github.com/sunfishcode/origin/blob/main/example-crates/basic/README.md
[no-std example]: https://github.com/sunfishcode/origin/blob/main/example-crates/no-std/README.md
[external-start example]: https://github.com/sunfishcode/origin/blob/main/example-crates/external-start/README.md
[origin-start example]: https://github.com/sunfishcode/origin/blob/main/example-crates/origin-start/README.md
[origin-start-no-alloc example]: https://github.com/sunfishcode/origin/blob/main/example-crates/origin-start-no-alloc/README.md
[origin-start-lto example]: https://github.com/sunfishcode/origin/blob/main/example-crates/origin-start-lto/README.md
[tiny example]: https://github.com/sunfishcode/origin/blob/main/example-crates/tiny/README.md
[Mustang]: https://github.com/sunfishcode/mustang#readme
[Eyra]: https://github.com/sunfishcode/eyra#readme
[Origin Studio]: https://github.com/sunfishcode/origin-studio#readme
[c-scape]: https://github.com/sunfishcode/c-ward/tree/main/c-scape#readme

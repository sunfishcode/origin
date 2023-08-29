This crate demonstrates the use of origin as a plain library API using
`no_std` and `no_main`.

This version uses `-nostartfiles` and origin is in control from the very
beginning. This uses origin in an "origin-start" configuration.

The resulting executables don't use any dynamic libraries, however by default
they do still depend on a dynamic linker. To enable fully static linking, the
current workaround is to disable PIE using
`RUSTFLAGS=-C target-feature=+crt-static -C relocation-model=static`.

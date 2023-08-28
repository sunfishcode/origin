This crate demonstrates the use of origin as a plain library API using
`no_std` and `no_main`.

This version uses `-nostartfiles` and origin is in control from the very
beginning. This uses origin in an "origin-start" configuration.

This crate must be built with an explicit `--target` argument, which may be
the same as the host, because the existence of `--target` prevents the
`-nostartfile` from being passed to build scripts.

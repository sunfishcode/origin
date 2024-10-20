# origin 0.23

## Changes

`program::abort` has been renamed to `program::trap`, and the
"panic-handler-abort" feature has been renamed to "panic-handler-trap".

# origin 0.22

## Changes

`program::at_exit` and `thread::at_exit` are now behind feature flags
"program-at-exit" and "thread-at-exit", respectively. They're enabled by
default, but many users disable the default features, so they may need to be
explicitly enabled if needed.

The "atomic-dbg-logger" and "env_logger" features no longer implicitly enable
the "fini-array" feature.

# origin 0.21

## Changes

Origin now supports stable Rust as well as nightly Rust, and nightly-only
features such as unwinding are now behind a "nightly" feature.

In "take-charge" mode, origin now defines `memcpy`, `memset` and other
functions needed by rustc and internal Rust libraries, so it's no longer
necessary to use `compiler_builtins` to provide these.

# origin 0.20

## Changes

The `origin-program`, `origin-thread`, and `origin-signal` features are now
combined into a single `take-charge` feature.

# origin 0.19

## Changes

`current_tls_addr` now has a `module` parameter instead of hard-coding the
module to be 1.

# origin 0.18

## Changes

The `set_thread_id` feature is now enabled unconditionally.

# origin 0.17

## Changes

The "thread" feature is no longer implied by the "origin-thread" feature, so
users using threads with "origin-thread" will need to also enable "thread"
explicitly.

The "alloc" feature is no longer implied by the "origin-thread" or "thread"
features, so users using program or thread destructors with "origin-thread"
will need to also enable "alloc" explicitly.

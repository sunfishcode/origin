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

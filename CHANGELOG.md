# origin 0.17

## Changes

The "thread" feature is no longer implied by the "origin-thread" feature, so
users using threads with "origin-thread" will need to also enable "thread"
explicitly.

The "alloc" feature is no longer implied by the "origin-thread" or "thread"
features, so users using program or thread destructors with "origin-thread"
will need to also enable "alloc" explicitly.

# origin 0.15

## Changes

The `create_thread` API has been redesigned to allow it and users of it
to avoid performing dynamic allocations. Instead of taking a boxed closure,
it now takes a function pointer and a list of pointer arguments to pass
to it. See #94 for details.

The default stack size has been increased, as several use cases were bumping
up against the limit. See #91 for details.

Thread and process destructor lists now use the `smallvec` crate to avoid
allocating. See #93 for details.

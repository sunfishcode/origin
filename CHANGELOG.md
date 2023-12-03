# origin 0.16

## Changes

The functions in the `thread` module have been renamed to remove the
redundant `thread` in their names. For example, `thread::create_thread` is
now just `thread::create`, and so on.

The `thread::id` function (formerly `thread::thread_id`), now returns an
`Option`, to allow it to indicate that the thread has exited, in which case
it no longer has an ID.

Origin now supports linking with code compiled with stack protection features,
such as `-fstack-protector`.

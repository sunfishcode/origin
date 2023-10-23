# origin 0.14

## Changes

`thread::create_thread` now requires the function arugment to implement `Send`,
which reflects the fact that it is sent to the newly-created thread.

## Features

The "init-fini-arrays" feature now enables two sub-features, "init-arrays" and
"fini-arrays" which can be enabled individually for use cases that only need one.

A new "atomic-dbg-logger" feature uses the atomic-dbg packages `log` implementation,
which is very minimal and not configurable, but which can print messages to stderr
in more configurations.

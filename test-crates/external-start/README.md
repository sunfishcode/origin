This crate demonstrates the use of origin as a plain library API using
`no_std` and `no_main`.

This version lets libc start up the program, but takes control as soon as it
can, and then has origin do everything else. This uses origin in an
"external-start" configuration.

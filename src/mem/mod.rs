//! Decide whether to use the optimized-for-speed or optimized-for-size
//! implementations of `memset` etc.

#[cfg(not(feature = "optimize_for_size"))]
mod fast;
#[cfg(feature = "optimize_for_size")]
mod small;

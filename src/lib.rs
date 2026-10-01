#![doc = core::include_str!("../README.md")]
#![warn(
    missing_docs,
    clippy::unwrap_in_result,
    clippy::unwrap_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::float_cmp,
    clippy::float_cmp_const,
    clippy::missing_panics_doc,
    clippy::todo
)]
#![no_std]
// `#[thread_local]` statics are unstable; required for per-thread shard
// selection on atomics-enabled wasm32 without pulling in `std`.
#![cfg_attr(
    all(target_arch = "wasm32", target_feature = "atomics"),
    feature(thread_local)
)]

#[cfg(all(test, target_arch = "wasm32"))]
mod tests;

#[cfg(target_arch = "wasm32")]
mod allocator;

#[cfg(target_arch = "wasm32")]
pub use allocator::WasmParallelAllocator;

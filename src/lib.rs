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

#[cfg(all(test, target_arch = "wasm32", target_feature = "atomics"))]
mod tests;

#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
mod allocator;

#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
pub use allocator::WasmParallelAllocator;

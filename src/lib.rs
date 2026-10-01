#[cfg(all(test, target_arch = "wasm32", target_feature = "atomics"))]
mod tests;

#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
mod allocator;

#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
pub use allocator::ensure_linked;

#[cfg(not(all(target_arch = "wasm32", target_feature = "atomics")))]
pub fn ensure_linked() {}

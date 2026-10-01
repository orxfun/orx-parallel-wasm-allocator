#[cfg(all(
    test,
    target_arch = "wasm32",
    target_feature = "atomics",
    feature = "wasm-browser"
))]
mod tests;

#[cfg(all(
    target_arch = "wasm32",
    target_feature = "atomics",
    feature = "wasm-browser"
))]
mod allocator;

#[cfg(all(
    target_arch = "wasm32",
    target_feature = "atomics",
    feature = "wasm-browser"
))]
pub use allocator::ensure_linked;

#[cfg(not(all(
    target_arch = "wasm32",
    target_feature = "atomics",
    feature = "wasm-browser"
)))]
pub fn ensure_linked() {}

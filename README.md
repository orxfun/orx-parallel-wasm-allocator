# orx-parallel-wasm-allocator

A proof-of-concept global allocator for atomics-enabled WebAssembly builds that use `orx-parallel` with shared memory. It is intentionally workload-specific, not a general-purpose allocator recommendation.

## Current design

On an atomics-enabled `wasm32` target, the crate installs a global allocator made of 64 independent Talc heaps. A thread-local slot assigns each wasm thread a shard on its first allocation. Each shard uses Talc's wasm binning strategy and `WasmGrowAndClaim` memory source, with a `RawSpinlock` protecting that shard's heap.

Each returned allocation stores its owning shard index immediately before the user pointer. This lets deallocation and reallocation return memory to the original shard even when an allocation is moved or freed by another worker. Shard collisions remain correct because each heap is locked, but they can reintroduce contention.

The `sharded` feature is enabled by default and selects 64 shards. Disable it to build the single-shard control; that keeps the same Talc wasm binning and metadata path while restoring one shared allocator lock.

Select the allocator once in the final WebAssembly crate with Rust's standard `#[global_allocator]` mechanism:

```rust
use orx_parallel_wasm_allocator::WasmParallelAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: WasmParallelAllocator = WasmParallelAllocator;
```

This compile-time declaration both selects the allocator and keeps it in the final wasm link graph. No exported function needs to call an initialization or linking helper.

## Scope and limitations

- Intended for atomics-enabled WebAssembly with shared memory and `orx-parallel` workloads, including browser WebAssembly.
- Does not change `orx-parallel` or provide a configurable allocator API.
- Uses a fixed shard count of 64; it is not yet configurable from the worker-pool size.
- Each shard obtains memory independently. This can increase retained heap space and fragmentation.
- Performance and safety validation is still a PoC: test allocator alignment, reallocation, cross-worker frees, memory growth, and OOM behavior before treating it as reusable infrastructure.

## Test

Please use the following commands to test the lib with sharding enabled:

```shell
RUSTC_BOOTSTRAP=1 \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=/tmp/orx-wasm-bindgen-0.2.129/bin/wasm-bindgen-test-runner \
RUSTFLAGS="-C target-feature=+atomics" \
cargo test --target wasm32-unknown-unknown --features sharded
```

and the following to test with a single shard configuration:

```shell
RUSTC_BOOTSTRAP=1 \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=/tmp/orx-wasm-bindgen-0.2.129/bin/wasm-bindgen-test-runner \
RUSTFLAGS="-C target-feature=+atomics" \
cargo test --target wasm32-unknown-unknown --no-default-features
```

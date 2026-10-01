# orx-parallel-wasm-allocator

`orx-parallel-wasm-allocator` is a specialized global allocator for memory-heavy, atomics-enabled WebAssembly workloads that use [`orx-parallel`](https://github.com/orxfun/orx-parallel). It is designed for parallel browser WebAssembly workflows where several workers allocate concurrently and the default allocator becomes a scalability bottleneck.

This is not a general-purpose WebAssembly allocator. Use it when your application needs shared-memory WebAssembly threads, uses `orx-parallel`, and benefits from independent allocation heaps. Benchmark your workload before adopting it in production.

## Why it exists

The allocator maintains independent Talc heaps, each protected by a `RawSpinlock`. A WebAssembly thread is assigned a shard on its first allocation. Allocations record their owning shard immediately before the user pointer, so a later deallocation or reallocation can return memory to the correct heap even when another worker performs it.

The default `sharded` feature creates 64 heaps. Disable the default features to build a single-shard control configuration. The single-shard configuration is useful for comparison, but it gives up the allocator's main contention-reduction strategy.

Independent heaps can increase retained memory and fragmentation. The shard count is currently fixed at 64 and is not derived from the worker-pool size.

## Usage

Add the dependency to the crate that produces the final WebAssembly module:

```toml
[dependencies]
orx-parallel-wasm-allocator = "0.1"
```

Declare the allocator once at crate scope:

```rust
use orx_parallel_wasm_allocator::WasmParallelAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: WasmParallelAllocator = WasmParallelAllocator;
```

The declaration must be in the final WebAssembly crate, such as a `wasm-bindgen` bindings crate. It selects the allocator at compile time and keeps the allocator crate in the final link graph.

The allocator is compiled only for `wasm32` builds with the `atomics` target feature. It does not replace `orx-parallel` runtime initialization: initialize the parallel runtime once in each worker before its first parallel computation, as described in the [`orx-parallel` WASM documentation](https://github.com/orxfun/orx-parallel/blob/main/docs/wasm.md).

## Browser requirements

Threaded browser WebAssembly also requires:

- a `wasm32-unknown-unknown` build with `+atomics` and shared-memory linker settings;
- `SharedArrayBuffer`, which requires `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`;
- per-worker initialization of the `orx-parallel` WASM runtime before parallel work begins.

The [`orx-parallel-wasm-demos` repository](https://github.com/orxfun/orx-parallel-wasm-demos) contains the build configuration for these requirements. Its [`vanilla-mem` TSP demo](https://github.com/orxfun/orx-parallel-wasm-demos/tree/main/tsp/vanilla-mem) shows the complete layout:

- [`computation`](https://github.com/orxfun/orx-parallel-wasm-demos/tree/main/tsp/vanilla-mem/computation) contains pure Rust computation deliberately designed to create memory pressure;
- [`wasm_bindings`](https://github.com/orxfun/orx-parallel-wasm-demos/tree/main/tsp/vanilla-mem/wasm_bindings) exposes the computation and declares this allocator once;
- [`app`](https://github.com/orxfun/orx-parallel-wasm-demos/tree/main/tsp/vanilla-mem/app) builds and runs the Vite browser application.

A hosted version of the demo is available at [orx-parallel-wasm-demo-tsp.pages.dev](https://orx-parallel-wasm-demo-tsp.pages.dev/).

## Testing

The allocator tests target atomics-enabled WebAssembly. The `wasm-bindgen-test-runner` version must match the `wasm-bindgen` version resolved in `Cargo.lock`; the current lockfile uses `0.2.129`. Install the matching runner if necessary:

```shell
cargo install wasm-bindgen-cli --version 0.2.129 --locked
```

Run the tests with the runner found on your `PATH`:

```shell
RUSTC_BOOTSTRAP=1 \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="$(command -v wasm-bindgen-test-runner)" \
RUSTFLAGS="-C target-feature=+atomics" \
cargo test --target wasm32-unknown-unknown --features sharded
```

For the single-shard comparison:

```shell
RUSTC_BOOTSTRAP=1 \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="$(command -v wasm-bindgen-test-runner)" \
RUSTFLAGS="-C target-feature=+atomics" \
cargo test --target wasm32-unknown-unknown --no-default-features
```

For the broader project context, see [`orx-parallel`](https://github.com/orxfun/orx-parallel), [`orx-parallel-wasm`](https://github.com/orxfun/orx-parallel-wasm), and the [`orx-parallel-wasm-demos`](https://github.com/orxfun/orx-parallel-wasm-demos) repository.

# orx-parallel-wasm-allocator

[![orx-parallel-wasm-allocator crate](https://img.shields.io/crates/v/orx-parallel-wasm-allocator.svg)](https://crates.io/crates/orx-parallel-wasm-allocator)
[![orx-parallel-wasm-allocator crate](https://img.shields.io/crates/d/orx-parallel-wasm-allocator.svg)](https://crates.io/crates/orx-parallel-wasm-allocator)
[![orx-parallel-wasm-allocator documentation](https://docs.rs/orx-parallel-wasm-allocator/badge.svg)](https://docs.rs/orx-parallel-wasm-allocator)

`orx-parallel-wasm-allocator` is a specialized global allocator for memory-heavy, atomics-enabled WebAssembly workloads that use [`orx-parallel`](https://github.com/orxfun/orx-parallel). It is designed for parallel browser WebAssembly workflows where several workers allocate concurrently and the default allocator becomes a scalability bottleneck.

This is not a general-purpose WebAssembly allocator. Use it when your application needs shared-memory WebAssembly threads, uses `orx-parallel`, and benefits from independent allocation heaps. Benchmark your workload before adopting it in production.

## Why it exists

Parallel WebAssembly does not automatically make every workload faster. For compute-heavy algorithms, the default allocator is often sufficient and a threaded implementation can scale well. The [`orx-parallel-wasm-demos/tsp/vanilla`](https://github.com/orxfun/orx-parallel-wasm-demos/tree/main/tsp/vanilla) example demonstrates this kind of workload ([live demo](https://orx-parallel-wasm-demo-tsp-vanilla.pages.dev/)).

Memory-heavy algorithms are different. The [`orx-parallel-wasm-demos/tsp/vanilla-mem`](https://github.com/orxfun/orx-parallel-wasm-demos/tree/main/tsp/vanilla-mem) example deliberately performs excessive allocation to create memory pressure. With the default allocator, allocator contention can become the bottleneck, so adding workers can make the parallel version slower than the single-threaded version.

This allocator addresses that specific failure mode by giving allocating WebAssembly threads independent heaps.

## When to use

*As a practical **rule of thumb**, try it when a multi-threaded run, for example with 4 threads, is unexpectedly slower than the 1-threaded run for the same workload.*

## Usage

Normally, enable the allocator through `orx-parallel`, which re-exports the
allocator from its `wasm-allocator` feature:

```toml
[dependencies]
orx-parallel = { version = "4.0", default-features = false, features = ["wasm", "wasm-allocator"] }
```

Use a direct dependency on `orx-parallel-wasm-allocator` only when the
allocator is needed without the `orx-parallel` integration:

```toml
[dependencies]
orx-parallel-wasm-allocator = "0.1"
```

Declare the allocator once at crate scope:

```rust ignore
use orx_parallel::WasmParallelAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: WasmParallelAllocator<64> = WasmParallelAllocator::<64>::new();
```

With the standalone dependency, import the type from
`orx_parallel_wasm_allocator` instead.

The shard count is a compile-time parameter. Use `WasmParallelAllocator::<64>::new()` for 64 shards or `WasmParallelAllocator::<1>::new()` for one shard.

The declaration must be in the final WebAssembly crate, such as a `wasm-bindgen` bindings crate. It selects the allocator at compile time and keeps the allocator crate in the final link graph.

The allocator API is available on `wasm32`; it is intended for builds with the `atomics` target feature and shared memory. It does not replace `orx-parallel` runtime initialization: initialize the parallel runtime once in each worker before its first parallel computation, as described in the [`orx-parallel` WASM documentation](https://github.com/orxfun/orx-parallel/blob/main/docs/wasm.md).

## Shards and worker count

A shard is an independent Talc heap protected by a `RawSpinlock`. A WebAssembly thread is assigned a shard on its first allocation. Allocations record their owning shard immediately before the user pointer, so a later deallocation or reallocation can return memory to the correct heap even when another worker performs it.

The const generic defaults to 64 heaps. Use `WasmParallelAllocator<1>` for a single-shard control configuration. The single-shard configuration is useful for comparison, but it gives up the allocator's main contention-reduction strategy. Independent heaps can increase retained memory and fragmentation.

A 64-shard allocator is valid for an `orx-parallel` pool with 8 workers. Shards are assigned lazily: a thread receives a shard only when it performs its first allocation. If exactly those 8 threads allocate, and no other threads allocate, the allocator will use 8 shards in either a 64-shard or an 8-shard configuration. In that narrow case, the two configurations should have essentially the same allocation contention and performance.

The configurations are not strictly identical. A 64-shard build still has a larger static shard table, and its shard-selection arithmetic uses a different constant, but those costs are tiny and occur mostly during initialization or a thread's first allocation. The meaningful difference is headroom: the 64-shard build avoids collisions if the main thread, runtime threads, or additional worker threads also allocate. A thread that is freed and later recreated also consumes another shard assignment over its lifetime.

An 8-shard build can be better when the application truly has only 8 allocating threads and memory efficiency matters, because it has fewer independent heaps and less opportunity for fragmentation or retained memory. It can be worse if more than 8 distinct threads allocate, because multiple threads then share a shard lock. Allocations freed by another thread still lock the owning shard in both configurations.

Therefore, B is not automatically faster than A. Choose 8 shards when the allocating-thread count is a stable, known limit and lower memory overhead is important; choose 64 when some extra memory overhead is acceptable and you want protection against unexpected or future allocating threads. Benchmark with the actual allocation pattern and choose the const generic that fits your application.

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

Run the tests with the runner found on your `PATH`. The suite covers the default 64-shard allocator and an explicit eight-shard allocator:

```shell
RUSTC_BOOTSTRAP=1 \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="$(command -v wasm-bindgen-test-runner)" \
RUSTFLAGS="-C target-feature=+atomics" \
cargo test --target wasm32-unknown-unknown
```

The single-shard configuration is `WasmParallelAllocator::<1>::new()`. Select it in the consuming crate's `#[global_allocator]` declaration when comparing one shard against a larger configuration.

For the broader project context, see [`orx-parallel`](https://github.com/orxfun/orx-parallel), [`orx-parallel-wasm`](https://github.com/orxfun/orx-parallel-wasm), and the [`orx-parallel-wasm-demos`](https://github.com/orxfun/orx-parallel-wasm-demos) repository.

## Contributing

Contributions are welcome! If you notice an error, have a question or think something could be improved, please open an [issue](https://github.com/orxfun/orx-parallel-wasm-allocator/issues/new) or create a PR.

## License

Dual-licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).

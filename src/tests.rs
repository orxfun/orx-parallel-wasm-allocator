use crate::allocator::WasmParallelAllocator;
use core::alloc::{GlobalAlloc, Layout};
use wasm_bindgen_test::wasm_bindgen_test;

#[cfg(feature = "sharded")]
use crate::allocator::{
    SHARD_COUNT, allocate_from_shard, current_shard, padded_layout, read_shard_index,
};

#[wasm_bindgen_test]
fn allocations_preserve_requested_alignment_and_bytes() {
    let allocator = WasmParallelAllocator;

    for alignment in [1, 2, 4, 8, 16, 64, 256, 4096] {
        let layout = Layout::from_size_align(37, alignment).unwrap();
        let allocation = unsafe { allocator.alloc(layout) };
        assert!(!allocation.is_null());
        assert_eq!(allocation as usize % alignment, 0);

        for index in 0..layout.size() {
            unsafe { allocation.add(index).write(index as u8) };
        }
        for index in 0..layout.size() {
            assert_eq!(unsafe { allocation.add(index).read() }, index as u8);
        }

        unsafe { allocator.dealloc(allocation, layout) };
    }
}

#[wasm_bindgen_test]
fn alloc_zeroed_initializes_the_requested_bytes() {
    let allocator = WasmParallelAllocator;
    let layout = Layout::from_size_align(257, 64).unwrap();
    let allocation = unsafe { allocator.alloc_zeroed(layout) };
    assert!(!allocation.is_null());

    for index in 0..layout.size() {
        assert_eq!(unsafe { allocation.add(index).read() }, 0);
    }

    unsafe { allocator.dealloc(allocation, layout) };
}

#[wasm_bindgen_test]
fn realloc_preserves_data_when_growing_and_shrinking() {
    let allocator = WasmParallelAllocator;
    let initial_layout = Layout::from_size_align(32, 64).unwrap();
    let allocation = unsafe { allocator.alloc(initial_layout) };
    assert!(!allocation.is_null());
    for index in 0..initial_layout.size() {
        unsafe { allocation.add(index).write((index ^ 0x5a) as u8) };
    }

    let grown = unsafe { allocator.realloc(allocation, initial_layout, 96) };
    assert!(!grown.is_null());
    for index in 0..initial_layout.size() {
        assert_eq!(unsafe { grown.add(index).read() }, (index ^ 0x5a) as u8);
    }

    let grown_layout = Layout::from_size_align(96, 64).unwrap();
    let shrunk = unsafe { allocator.realloc(grown, grown_layout, 16) };
    assert!(!shrunk.is_null());
    for index in 0..16 {
        assert_eq!(unsafe { shrunk.add(index).read() }, (index ^ 0x5a) as u8);
    }

    unsafe { allocator.dealloc(shrunk, Layout::from_size_align(16, 64).unwrap()) };
}

#[wasm_bindgen_test]
fn failed_allocation_and_reallocation_do_not_corrupt_existing_data() {
    let allocator = WasmParallelAllocator;
    let impossible_layout = Layout::from_size_align(isize::MAX as usize, 1).unwrap();
    assert!(unsafe { allocator.alloc(impossible_layout) }.is_null());

    let layout = Layout::from_size_align(8, 1).unwrap();
    let allocation = unsafe { allocator.alloc(layout) };
    assert!(!allocation.is_null());
    unsafe { allocation.write_bytes(0xa5, layout.size()) };

    let failed = unsafe { allocator.realloc(allocation, layout, isize::MAX as usize) };
    assert!(failed.is_null());
    for index in 0..layout.size() {
        assert_eq!(unsafe { allocation.add(index).read() }, 0xa5);
    }

    unsafe { allocator.dealloc(allocation, layout) };
}

#[cfg(feature = "sharded")]
#[wasm_bindgen_test]
fn deallocation_uses_the_recorded_owner_shard() {
    let allocator = WasmParallelAllocator;
    let current = current_shard();
    let owner = (current + 1) % SHARD_COUNT;
    let layout = Layout::from_size_align(48, 16).unwrap();
    let Some(padded) = padded_layout(layout) else {
        panic!("valid test layout should be paddable");
    };
    let allocation = allocate_from_shard(owner, padded);
    assert!(!allocation.is_null());
    assert_eq!(unsafe { read_shard_index(allocation) }, owner);

    unsafe { allocator.dealloc(allocation, layout) };

    let allocation_again = allocate_from_shard(owner, padded);
    assert!(!allocation_again.is_null());
    unsafe { allocator.dealloc(allocation_again, layout) };
}

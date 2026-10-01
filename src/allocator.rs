use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

const UNASSIGNED_SHARD: usize = usize::MAX;
const HEADER_SIZE: usize = core::mem::size_of::<usize>();

type Shard = talc::sync::TalcLock<
    spinning_top::RawSpinlock,
    talc::wasm::WasmGrowAndClaim,
    talc::wasm::WasmBinning,
>;

#[thread_local]
static THREAD_SHARD: Cell<usize> = const { Cell::new(UNASSIGNED_SHARD) };

/// A sharded global allocator for atomics-enabled WebAssembly.
///
/// Each shard owns an independent Talc heap protected by a spinlock. The
/// `SHARD_COUNT` const generic controls the number of heaps and defaults to 64.
/// Use a smaller value when the number of allocating threads is a known limit
/// and lower retained memory is more important than additional concurrency
/// headroom.
pub struct WasmParallelAllocator<const SHARD_COUNT: usize = 64> {
    next_shard: AtomicUsize,
    shards: [Shard; SHARD_COUNT],
}

impl<const SHARD_COUNT: usize> WasmParallelAllocator<SHARD_COUNT> {
    /// Creates an allocator with `SHARD_COUNT` independent heaps.
    ///
    /// # Panics
    ///
    /// Panics at compile time when `SHARD_COUNT` is zero.
    pub const fn new() -> Self {
        assert!(SHARD_COUNT > 0);
        Self {
            next_shard: AtomicUsize::new(0),
            shards: [const { Shard::new(talc::wasm::WasmGrowAndClaim) }; SHARD_COUNT],
        }
    }
}

impl<const SHARD_COUNT: usize> Default for WasmParallelAllocator<SHARD_COUNT> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<const SHARD_COUNT: usize> GlobalAlloc for WasmParallelAllocator<SHARD_COUNT> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match padded_layout(layout) {
            Some(padded) => {
                let shard_index = self.current_shard();
                self.allocate_from_shard(shard_index, padded)
            }
            None => ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(padded) = padded_layout(layout) else {
            return;
        };
        // SAFETY: The caller must provide a pointer and layout returned by this allocator.
        let shard_index = unsafe { read_shard_index(ptr) };
        let Some(shard) = self.shards.get(shard_index) else {
            return;
        };
        // SAFETY: `padded.prefix` points from the user pointer to the base returned by this allocator.
        let base = unsafe { ptr.sub(padded.prefix) };
        // SAFETY: `base` and `padded.layout` describe the live allocation previously returned by this shard.
        unsafe { shard.lock().deallocate(base, padded.layout) };
    }

    // SAFETY: The caller must provide a valid allocation layout as required by `GlobalAlloc`.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `layout` is forwarded from the `GlobalAlloc` caller and therefore satisfies the same contract.
        let allocation = unsafe { self.alloc(layout) };
        if !allocation.is_null() {
            // SAFETY: The allocator returned at least `layout.size()` writable bytes at `allocation`.
            unsafe { allocation.write_bytes(0, layout.size()) };
        }
        allocation
    }

    // SAFETY: The caller must provide a live allocation and its original layout as required by `GlobalAlloc`.
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let Some(new_layout) = Layout::from_size_align(new_size, layout.align()).ok() else {
            return ptr::null_mut();
        };
        let Some(padded) = padded_layout(new_layout) else {
            return ptr::null_mut();
        };
        // SAFETY: The caller must provide a pointer and layout returned by this allocator.
        let shard_index = unsafe { read_shard_index(ptr) };
        let replacement = self.allocate_from_shard(shard_index, padded);
        if replacement.is_null() {
            return replacement;
        }

        // SAFETY: Both pointers refer to distinct live allocations, and the copied length fits in both.
        unsafe { ptr::copy_nonoverlapping(ptr, replacement, layout.size().min(new_size)) };
        // SAFETY: `ptr` is the live allocation supplied by the caller, with its original layout.
        unsafe { self.dealloc(ptr, layout) };
        replacement
    }
}

impl<const SHARD_COUNT: usize> WasmParallelAllocator<SHARD_COUNT> {
    pub(super) fn current_shard(&self) -> usize {
        let shard_index = THREAD_SHARD.get();
        if shard_index < SHARD_COUNT {
            return shard_index;
        }

        let shard_index = self.next_shard.fetch_add(1, Ordering::Relaxed) % SHARD_COUNT;
        THREAD_SHARD.set(shard_index);
        shard_index
    }

    pub(super) fn allocate_from_shard(&self, shard_index: usize, padded: PaddedLayout) -> *mut u8 {
        let Some(shard) = self.shards.get(shard_index) else {
            return ptr::null_mut();
        };
        // SAFETY: `padded.layout` is a valid layout produced by `padded_layout`, and the shard lock guards its heap.
        let Some(base) = (unsafe { shard.lock().allocate(padded.layout) }) else {
            return ptr::null_mut();
        };
        // SAFETY: The allocation contains the reserved prefix and the resulting pointer remains within its bounds.
        let allocation = unsafe { base.as_ptr().add(padded.prefix) };
        // SAFETY: `padded.prefix` reserves space for the header, and the base layout is aligned for `usize`.
        unsafe {
            allocation
                .sub(HEADER_SIZE)
                .cast::<usize>()
                .write_unaligned(shard_index);
        }
        allocation
    }
}

#[derive(Clone, Copy)]
pub(super) struct PaddedLayout {
    layout: Layout,
    prefix: usize,
}

pub(super) fn padded_layout(layout: Layout) -> Option<PaddedLayout> {
    let prefix = HEADER_SIZE
        .checked_add(layout.align() - 1)
        .map(|size| size & !(layout.align() - 1))?;
    let size = prefix.checked_add(layout.size().max(1))?;
    let layout =
        Layout::from_size_align(size, layout.align().max(core::mem::align_of::<usize>())).ok()?;
    Some(PaddedLayout { layout, prefix })
}

// SAFETY: The caller must pass a pointer previously returned by this allocator and still owned by it.
pub(super) unsafe fn read_shard_index(ptr: *mut u8) -> usize {
    // SAFETY: The allocator stores the shard index immediately before every returned pointer.
    unsafe { ptr.sub(HEADER_SIZE).cast::<usize>().read_unaligned() }
}

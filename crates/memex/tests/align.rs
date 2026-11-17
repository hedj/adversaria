//! The alignment of two versions of a document (memex::docs::align), by
//! the memory it takes. A counting allocator, for this test binary only,
//! measures the most held at once.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static HELD: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let held = HELD.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
        PEAK.fetch_max(held, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HELD.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

#[test]
fn aligning_two_long_documents_takes_memory_in_proportion_to_their_length() {
    // Catches an alignment that keeps a table of every pair of blocks:
    // 3000 blocks a side took 36 MB, and 20 000 a side would take 1.6 GB.
    // Every third block of the new version is new, so the alignment is
    // neither empty nor whole.
    let old: Vec<u32> = (0..3000).collect();
    let new: Vec<u32> = (0..3000)
        .map(|i| if i % 3 == 0 { 10_000 + i } else { i })
        .collect();
    let before = HELD.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let pairs = memex::docs::align(&old, &new);
    let most = PEAK.load(Ordering::SeqCst) - before;
    assert_eq!(pairs.len(), 2000);
    assert!(most < 1_000_000, "{most} bytes held at once");
}

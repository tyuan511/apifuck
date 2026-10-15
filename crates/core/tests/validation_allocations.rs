//! Keep this as a single-test binary: schema caches must start cold, and the
//! allocator measures the whole process. These are requested Rust heap bytes,
//! not RSS or the desktop's physical footprint.
use apifuck_core::{
    DocumentKind, collection,
    mcp::{Service, Workspace},
};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

fn allocated(size: usize) {
    LIVE.fetch_add(size, Relaxed);
    ALLOCATED.fetch_add(size, Relaxed);
}

// SAFETY: Every operation forwards the original pointer/layout to System.
// Counters do not allocate and only record successful allocations.
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            allocated(size);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: Meter = Meter;

#[test]
fn validation_keeps_cold_memory_and_repeated_tool_allocations_bounded() {
    const MIB: usize = 1024 * 1024;
    let root = json!({"opencollection":"1.0.0","info":{"name":"Memory probe"}});
    let baseline = LIVE.load(Relaxed);
    collection::validate(&root, DocumentKind::Collection).unwrap();
    let root_bytes = LIVE.load(Relaxed).saturating_sub(baseline);
    // An empty project must not initialize every document validator (~12 MiB).
    assert!(
        root_bytes < 8 * MIB,
        "root validation retained {root_bytes} bytes"
    );

    for (name, kind) in [
        ("folder", DocumentKind::Folder),
        ("environment", DocumentKind::Environment),
        ("http", DocumentKind::Http),
        ("graphql", DocumentKind::Graphql),
        ("websocket", DocumentKind::WebSocket),
        ("http", DocumentKind::Other),
    ] {
        let value = serde_yaml::from_str(&collection::template(name, name).unwrap()).unwrap();
        collection::validate(&value, kind).unwrap();
    }
    let all_bytes = LIVE.load(Relaxed).saturating_sub(baseline);
    // Losing the embedded Draft-07 declaration previously retained ~78 MiB.
    assert!(
        all_bytes < 24 * MIB,
        "document validation retained {all_bytes} bytes"
    );

    let temp = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let (service, _receiver) =
        Service::new(temp.path().join("mcp.json"), runtime.handle().clone()).unwrap();
    let mut workspace = Workspace::default();
    let arguments = json!({});
    // Read missing history in the temporary directory: no project, HTTP or
    // background tasks, and no error backtrace allocations in the sample.
    assert_eq!(
        service
            .call("list_projects", &arguments, &mut workspace)
            .unwrap(),
        json!([])
    );
    let before = ALLOCATED.load(Relaxed);
    for _ in 0..100 {
        let _ = service
            .call("list_projects", &arguments, &mut workspace)
            .unwrap();
    }
    let repeated_bytes = ALLOCATED.load(Relaxed) - before;
    assert!(
        repeated_bytes < 128 * 1024,
        "100 warmed tool calls allocated {repeated_bytes} bytes; check schema recompilation"
    );
    eprintln!(
        "validation allocations: root={root_bytes} B, all document kinds={all_bytes} B, 100 warmed MCP calls={repeated_bytes} B"
    );
}

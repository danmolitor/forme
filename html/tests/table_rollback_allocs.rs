//! Fails-first guard for Fix 3-B: a table whose rows all fit on the page and
//! contain no forced breaks must NOT deep-clone the page cursor per cell.
//!
//! `layout_table_row` snapshots `cursor.clone()` before each cell as a rollback
//! checkpoint for the page-break case. The cursor owns `elements:
//! Vec<LayoutElement>` — every element on the page so far — so the clone deep-
//! copies the growing page vec once per cell. dhat found this single site is
//! ~76% of all allocated bytes on invoice-50p. When the whole row provably fits
//! and has no forced break, no cell can break, so the checkpoint is dead and
//! must be elided.
//!
//! This renders a single-page table with many small cells (so `cursor.elements`
//! grows large while glyph/base allocations stay modest — maximizing the
//! cursor-clone signal). Its own binary with a counting allocator, armed around
//! the render.
//!   - before the fix: O(cells × elements-per-page) clones → allocs well over
//!     the bound
//!   - after the fix:  ~0 checkpoint clones → allocs under the bound

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

struct Counting;
static ARMED: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.load(Relaxed) {
            ALLOCS.fetch_add(1, Relaxed);
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
    }
}

#[global_allocator]
static A: Counting = Counting;

/// A single-page table: 40 rows × 8 cells, 6pt text, tight rows — fits well
/// within a Letter page and has no forced breaks, so every per-cell rollback
/// checkpoint is dead.
fn fitting_table_html() -> String {
    let mut s = String::from(
        "<html><body style=\"font-size:6px\"><table style=\"border-collapse:collapse\">",
    );
    for r in 0..40 {
        s.push_str("<tr>");
        for c in 0..8 {
            s.push_str(&format!(
                "<td style=\"border:1px solid #000;padding:1px\">r{r}c{c}</td>"
            ));
        }
        s.push_str("</tr>");
    }
    s.push_str("</table></body></html>");
    s
}

#[test]
fn fitting_table_rows_skip_cursor_checkpoint() {
    let html = fitting_table_html();
    let opts = forme_pdf_html::HtmlOptions::default();

    ARMED.store(true, Relaxed);
    let out = forme_pdf_html::render_html(&html, &opts).expect("render");
    ARMED.store(false, Relaxed);
    std::hint::black_box(&out);

    let n = ALLOCS.load(Relaxed);
    // Measured: ~617k allocations on main (dead per-cell cursor clones
    // dominate), ~88k once they are elided. The bound sits between with wide
    // margin on both sides — fails on main, passes after the fix, and tolerates
    // minor allocation drift without going false either way.
    assert!(
        n <= 200_000,
        "rendering a fits-only table did {n} allocations; per-cell cursor \
         checkpoints are not being elided"
    );
}

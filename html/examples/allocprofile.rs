//! Allocation profile for the large-document investigation. A counting global
//! allocator wraps the system allocator and tracks total allocation count,
//! current live bytes, and peak live bytes across a render — answering whether
//! the >1GB peak on a 500-page render is the retained layout tree (peak ≈ final
//! live bytes) or transient churn (allocs ≫ peak, peak released before exit).
//!
//! Run: cargo run --release --example allocprofile -- benchmarks/corpus/<doc>.html

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            ALLOCS.fetch_add(1, Relaxed);
            let cur = CUR.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(cur, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        CUR.fetch_sub(l.size(), Relaxed);
    }
}

#[global_allocator]
static A: Counting = Counting;

fn main() {
    let path = std::env::args().nth(1).expect("usage: allocprofile <doc.html>");
    let html = std::fs::read_to_string(&path).expect("read");
    let opts = forme_pdf_html::HtmlOptions::default();
    let out = forme_pdf_html::render_html(&html, &opts).expect("render");
    let mb = |b: usize| b as f64 / 1_048_576.0;
    eprintln!(
        "ALLOCPROFILE {path}: allocs={} peak_live={:.0}MB final_live={:.0}MB pdf={:.1}KB",
        ALLOCS.load(Relaxed),
        mb(PEAK.load(Relaxed)),
        mb(CUR.load(Relaxed)),
        out.pdf.len() as f64 / 1024.0,
    );
}

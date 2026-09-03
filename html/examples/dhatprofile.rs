//! Heap-profile a render with dhat, attributing every allocation to a call-site
//! backtrace (Fix 3, allocation-churn investigation). The existing
//! `allocprofile` example gives totals only (allocs / peak / final live); this
//! one answers *where* — which call sites dominate by allocation COUNT (churn)
//! and by BYTES (peak). dhat writes `dhat-heap.json`; rank the sites with
//! `node benchmarks/harness/dhat-top.mjs dhat-heap.json`, or load it in the
//! online DHAT viewer (https://nnethercote.github.io/dh_view/dh_view.html).
//!
//! Run: cargo run --release --example dhatprofile -- benchmarks/corpus/<doc>.html
//!
//! Note: dhat instruments every alloc/free, so wall time here is NOT a perf
//! measurement — read counts and bytes, not the clock.

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: dhatprofile <doc.html>");

    // Start profiling AFTER reading the file so corpus I/O doesn't pollute the
    // top sites — we only want the render's allocations.
    let html = std::fs::read_to_string(&path).expect("read");
    let _profiler = dhat::Profiler::new_heap();

    let opts = forme_pdf_html::HtmlOptions::default();
    let out = forme_pdf_html::render_html(&html, &opts).expect("render");

    // Keep the output alive past the profiler drop point so nothing is
    // optimized away; report size for cross-checking against allocprofile.
    eprintln!(
        "DHATPROFILE {path}: pdf={:.1}KB (see dhat-heap.json)",
        out.pdf.len() as f64 / 1024.0,
    );
}

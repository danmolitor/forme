//! Fails-first guard for Fix 3 (allocation churn): cloning a run of glyphs that
//! share a font family must NOT heap-allocate once per glyph for `font_family`.
//!
//! dhat identified `PositionedGlyph::clone` → its `font_family: String` as the
//! #1 allocation site by COUNT — 2.65M allocations (29% of all) on invoice-50p,
//! because every glyph owns its own copy of the family string. Making the field
//! a shared `Arc<str>` turns clone into a refcount bump.
//!
//! This test is its own binary with a private counting global allocator, so the
//! count is not polluted by other tests. It arms the counter around a single
//! `TextLine::clone` of 100 same-family glyphs:
//!   - `String`  → ~1 (glyphs Vec) + 100 (per-glyph family) ≈ 101 allocations → FAILS
//!   - `Arc<str>`→ ~1 (glyphs Vec)                          ≈ 1   allocations → PASSES

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

use forme::layout::{PositionedGlyph, TextLine};
use forme::style::{Color, FontStyle, TextDecoration};

fn glyph(family: &str) -> PositionedGlyph {
    PositionedGlyph {
        glyph_id: 42,
        x_offset: 0.0,
        y_offset: 0.0,
        x_advance: 5.0,
        font_size: 12.0,
        font_family: family.into(),
        font_weight: 400,
        font_style: FontStyle::Normal,
        char_value: 'a',
        color: Some(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }),
        href: None,
        text_decoration: TextDecoration::None,
        letter_spacing: 0.0,
        cluster_text: None,
    }
}

#[test]
fn cloning_glyphs_shares_font_family() {
    // 100 glyphs, all the same family (the common case: one run, one font).
    let glyphs: Vec<PositionedGlyph> = (0..100).map(|_| glyph("Helvetica")).collect();
    let line = TextLine {
        x: 0.0,
        y: 0.0,
        glyphs,
        width: 500.0,
        height: 14.0,
        word_spacing: 0.0,
    };

    ARMED.store(true, Relaxed);
    let cloned = line.clone();
    ARMED.store(false, Relaxed);
    std::hint::black_box(&cloned);

    let n = ALLOCS.load(Relaxed);
    // Cloning the line allocates the glyphs Vec (1). With a shared family it
    // should add nothing per glyph; with an owned String it adds ~100. Allow a
    // little slack for allocator bookkeeping, but stay far below per-glyph.
    assert!(
        n <= 10,
        "cloning a 100-glyph line did {n} allocations; font_family is not shared \
         (expected ~1 with Arc<str>, ~101 with String)"
    );
}

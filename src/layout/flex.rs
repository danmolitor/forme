//! # Flex Layout Utilities
//!
//! Helper functions for the flexbox algorithm. The main flex logic lives
//! in the layout engine's `layout_flex_row` method. This module provides
//! the lower-level distribution calculations.

/// Distribute remaining space among items based on flex-grow factors.
pub fn distribute_grow(items: &mut [(f64, f64)], remaining: f64) {
    // items: [(current_width, flex_grow)]
    let total_grow: f64 = items.iter().map(|(_, g)| g).sum();
    if total_grow <= 0.0 || remaining <= 0.0 {
        return;
    }
    for (width, grow) in items.iter_mut() {
        *width += remaining * (*grow / total_grow);
    }
}

/// Shrink items to fit within available space based on flex-shrink factors.
pub fn distribute_shrink(items: &mut [(f64, f64)], overflow: f64) {
    // items: [(current_width, flex_shrink)]
    let total_shrink_weighted: f64 = items.iter().map(|(w, s)| w * s).sum();
    if total_shrink_weighted <= 0.0 || overflow >= 0.0 {
        return;
    }
    let overflow = overflow.abs();
    for (width, shrink) in items.iter_mut() {
        let factor = (*width * *shrink) / total_shrink_weighted;
        *width -= overflow * factor;
        *width = width.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grow_distribution() {
        let mut items = vec![(100.0, 1.0), (100.0, 2.0)];
        distribute_grow(&mut items, 90.0);
        assert!((items[0].0 - 130.0).abs() < 0.01);
        assert!((items[1].0 - 160.0).abs() < 0.01);
    }

    #[test]
    fn test_shrink_distribution() {
        let mut items = vec![(200.0, 1.0), (100.0, 1.0)];
        distribute_shrink(&mut items, -60.0);
        // 200 gets shrunk more because it's wider
        assert!(items[0].0 < 200.0);
        assert!(items[1].0 < 100.0);
        assert!((items[0].0 + items[1].0 - 240.0).abs() < 0.01);
    }
}

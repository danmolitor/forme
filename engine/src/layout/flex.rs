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

/// A single line of items in a wrapping flex row.
#[derive(Debug, Clone)]
pub struct WrapLine {
    /// Index of the first item in this line.
    pub start: usize,
    /// One past the last item (exclusive end).
    pub end: usize,
}

/// Partition items into wrap lines based on available width.
/// Always adds at least one item per line (prevents infinite loops on oversized items).
pub fn partition_into_lines(
    base_widths: &[f64],
    column_gap: f64,
    available_width: f64,
) -> Vec<WrapLine> {
    if base_widths.is_empty() {
        return vec![];
    }

    let mut lines = Vec::new();
    let mut line_start = 0;
    let mut line_width = 0.0;

    for (i, &w) in base_widths.iter().enumerate() {
        let needed = if i == line_start { w } else { column_gap + w };
        if i > line_start && line_width + needed > available_width {
            lines.push(WrapLine {
                start: line_start,
                end: i,
            });
            line_start = i;
            line_width = w;
        } else {
            line_width += needed;
        }
    }

    // Close the last line
    if line_start < base_widths.len() {
        lines.push(WrapLine {
            start: line_start,
            end: base_widths.len(),
        });
    }

    lines
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

    #[test]
    fn test_partition_single_line_fits() {
        let widths = vec![100.0, 100.0, 100.0];
        let lines = partition_into_lines(&widths, 10.0, 400.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start, 0);
        assert_eq!(lines[0].end, 3);
    }

    #[test]
    fn test_partition_two_line_split() {
        // 3 items × 100pt + 2 gaps × 10pt = 320pt; available = 250pt
        let widths = vec![100.0, 100.0, 100.0];
        let lines = partition_into_lines(&widths, 10.0, 250.0);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start, 0);
        assert_eq!(lines[0].end, 2); // first two fit: 100 + 10 + 100 = 210 <= 250
        assert_eq!(lines[1].start, 2);
        assert_eq!(lines[1].end, 3);
    }

    #[test]
    fn test_partition_oversized_item() {
        // Single item wider than available — must still get its own line
        let widths = vec![500.0];
        let lines = partition_into_lines(&widths, 10.0, 200.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start, 0);
        assert_eq!(lines[0].end, 1);
    }

    #[test]
    fn test_partition_empty_input() {
        let lines = partition_into_lines(&[], 10.0, 200.0);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_partition_exact_fit() {
        // 2 items × 100pt + 1 gap × 10pt = 210pt; available = 210pt — should fit on one line
        let widths = vec![100.0, 100.0];
        let lines = partition_into_lines(&widths, 10.0, 210.0);
        assert_eq!(lines.len(), 1);
    }
}

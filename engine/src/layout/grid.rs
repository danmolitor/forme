//! # CSS Grid Layout
//!
//! Implements a subset of CSS Grid for 2D layouts within the page-native
//! layout engine. Supports:
//! - Fixed (pt), fractional (fr), and auto track sizing
//! - MinMax track sizes
//! - Explicit grid placement (column/row start/end/span)
//! - Auto-placement (row-major sparse)
//! - Row/column gap
//! - Page breaks at row boundaries

use crate::style::{GridPlacement, GridTrackSize};

/// Resolved grid item placement in the grid.
#[derive(Debug, Clone)]
pub struct GridItemPlacement {
    /// Index of the child node.
    pub child_index: usize,
    /// Column start (0-based).
    pub col_start: usize,
    /// Column end (exclusive, 0-based).
    pub col_end: usize,
    /// Row start (0-based).
    pub row_start: usize,
    /// Row end (exclusive, 0-based).
    pub row_end: usize,
}

/// Resolve track sizes to concrete widths/heights in points.
///
/// Algorithm:
/// 1. Fixed tracks → exact size
/// 2. Auto tracks → `content_sizes[i]` (intrinsic content size)
/// 3. Fr tracks → distribute remaining space proportionally
/// 4. MinMax → clamp between resolved min and max
pub fn resolve_tracks(
    template: &[GridTrackSize],
    available_space: f64,
    gap: f64,
    content_sizes: &[f64],
) -> Vec<f64> {
    if template.is_empty() {
        return vec![];
    }

    let total_gap = if template.len() > 1 {
        gap * (template.len() - 1) as f64
    } else {
        0.0
    };
    let space_after_gaps = (available_space - total_gap).max(0.0);

    let mut sizes = vec![0.0_f64; template.len()];
    let mut remaining = space_after_gaps;
    let mut total_fr = 0.0_f64;

    // First pass: resolve fixed and auto tracks
    for (i, track) in template.iter().enumerate() {
        match track {
            GridTrackSize::Pt(pts) => {
                sizes[i] = *pts;
                remaining -= pts;
            }
            GridTrackSize::Auto => {
                let content = content_sizes.get(i).copied().unwrap_or(0.0);
                sizes[i] = content;
                remaining -= content;
            }
            GridTrackSize::Fr(fr) => {
                total_fr += fr;
            }
            GridTrackSize::MinMax(min, max) => {
                let min_val = resolve_single_track(min, 0.0);
                let max_val = resolve_single_track(max, space_after_gaps);
                let content = content_sizes.get(i).copied().unwrap_or(0.0);
                let val = content.max(min_val).min(max_val);
                sizes[i] = val;
                remaining -= val;
            }
        }
    }

    // Second pass: distribute remaining space to fr tracks
    remaining = remaining.max(0.0);
    if total_fr > 0.0 {
        let fr_unit = remaining / total_fr;
        for (i, track) in template.iter().enumerate() {
            if let GridTrackSize::Fr(fr) = track {
                sizes[i] = fr * fr_unit;
            }
        }
    }

    sizes
}

/// Resolve a single track size to a point value (for MinMax bounds).
fn resolve_single_track(track: &GridTrackSize, available: f64) -> f64 {
    match track {
        GridTrackSize::Pt(pts) => *pts,
        GridTrackSize::Fr(fr) => fr * available, // approximation
        GridTrackSize::Auto => 0.0,
        GridTrackSize::MinMax(min, _) => resolve_single_track(min, available),
    }
}

/// Place items in the grid using explicit placement + auto-placement.
///
/// Items with explicit `grid_placement` are placed first. Remaining items
/// fill the grid left-to-right, top-to-bottom (row-major sparse).
pub fn place_items(
    placements: &[Option<&GridPlacement>],
    num_columns: usize,
) -> Vec<GridItemPlacement> {
    if num_columns == 0 {
        return vec![];
    }

    let num_items = placements.len();
    let max_rows = num_items.div_ceil(num_columns) + num_items; // generous upper bound

    // Occupancy grid: true = occupied
    let mut occupied = vec![vec![false; num_columns]; max_rows];
    let mut result = Vec::with_capacity(num_items);

    // Phase 1: Place explicitly positioned items
    for (i, placement) in placements.iter().enumerate() {
        if let Some(gp) = placement {
            if gp.column_start.is_some() || gp.row_start.is_some() {
                let col_start = gp
                    .column_start
                    .map(|c| (c - 1).max(0) as usize)
                    .unwrap_or(0);
                let row_start = gp.row_start.map(|r| (r - 1).max(0) as usize).unwrap_or(0);

                let col_span = if let (Some(cs), Some(ce)) = (gp.column_start, gp.column_end) {
                    ((ce - cs).max(1)) as usize
                } else {
                    gp.column_span.unwrap_or(1) as usize
                };

                let row_span = if let (Some(rs), Some(re)) = (gp.row_start, gp.row_end) {
                    ((re - rs).max(1)) as usize
                } else {
                    gp.row_span.unwrap_or(1) as usize
                };

                let col_end = (col_start + col_span).min(num_columns);
                let row_end = row_start + row_span;

                // Mark cells as occupied
                for r in row_start..row_end {
                    for c in col_start..col_end {
                        if r < occupied.len() && c < num_columns {
                            occupied[r][c] = true;
                        }
                    }
                }

                result.push(GridItemPlacement {
                    child_index: i,
                    col_start,
                    col_end,
                    row_start,
                    row_end,
                });
            }
        }
    }

    // Phase 2: Auto-place remaining items (row-major sparse)
    let mut auto_row = 0;
    let mut auto_col = 0;

    for (i, placement) in placements.iter().enumerate() {
        let is_explicit = if let Some(gp) = placement {
            gp.column_start.is_some() || gp.row_start.is_some()
        } else {
            false
        };

        if is_explicit {
            continue;
        }

        let col_span = placement.and_then(|gp| gp.column_span).unwrap_or(1) as usize;
        let row_span = placement.and_then(|gp| gp.row_span).unwrap_or(1) as usize;

        // Find next available slot
        loop {
            if auto_col + col_span > num_columns {
                auto_col = 0;
                auto_row += 1;
            }

            // Grow occupancy grid if needed
            while auto_row + row_span > occupied.len() {
                occupied.push(vec![false; num_columns]);
            }

            // Check if slot is free
            let mut fits = auto_col + col_span <= num_columns;
            if fits {
                'check: for row in occupied.iter().skip(auto_row).take(row_span) {
                    for &cell in row.iter().skip(auto_col).take(col_span) {
                        if cell {
                            fits = false;
                            break 'check;
                        }
                    }
                }
            }

            if fits {
                break;
            }

            auto_col += 1;
        }

        // Place the item
        let col_end = auto_col + col_span;
        let row_end = auto_row + row_span;

        for r in auto_row..row_end {
            for c in auto_col..col_end {
                if r < occupied.len() && c < num_columns {
                    occupied[r][c] = true;
                }
            }
        }

        result.push(GridItemPlacement {
            child_index: i,
            col_start: auto_col,
            col_end,
            row_start: auto_row,
            row_end,
        });

        auto_col = col_end;
    }

    result
}

/// Compute the number of rows needed based on item placements.
pub fn compute_num_rows(placements: &[GridItemPlacement]) -> usize {
    placements.iter().map(|p| p.row_end).max().unwrap_or(0)
}

/// Compute the x-offset for a column, accounting for gaps.
pub fn column_x_offset(col: usize, col_widths: &[f64], gap: f64) -> f64 {
    let mut x = 0.0;
    for c in 0..col {
        x += col_widths.get(c).copied().unwrap_or(0.0);
        x += gap;
    }
    x
}

/// Compute the width of a multi-column span.
pub fn span_width(col_start: usize, col_end: usize, col_widths: &[f64], gap: f64) -> f64 {
    let mut w = 0.0;
    for c in col_start..col_end {
        w += col_widths.get(c).copied().unwrap_or(0.0);
        if c > col_start {
            w += gap;
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_tracks_fixed() {
        let tracks = vec![GridTrackSize::Pt(100.0), GridTrackSize::Pt(200.0)];
        let sizes = resolve_tracks(&tracks, 400.0, 0.0, &[]);
        assert_eq!(sizes.len(), 2);
        assert!((sizes[0] - 100.0).abs() < 0.001);
        assert!((sizes[1] - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_resolve_tracks_fr() {
        let tracks = vec![
            GridTrackSize::Pt(100.0),
            GridTrackSize::Fr(1.0),
            GridTrackSize::Fr(2.0),
        ];
        let sizes = resolve_tracks(&tracks, 400.0, 0.0, &[]);
        assert_eq!(sizes.len(), 3);
        assert!((sizes[0] - 100.0).abs() < 0.001);
        assert!((sizes[1] - 100.0).abs() < 0.001); // 1fr = 300/3 = 100
        assert!((sizes[2] - 200.0).abs() < 0.001); // 2fr = 300*2/3 = 200
    }

    #[test]
    fn test_resolve_tracks_with_gap() {
        let tracks = vec![GridTrackSize::Fr(1.0), GridTrackSize::Fr(1.0)];
        let sizes = resolve_tracks(&tracks, 210.0, 10.0, &[]);
        assert_eq!(sizes.len(), 2);
        // 210 - 10 (gap) = 200, split equally = 100 each
        assert!((sizes[0] - 100.0).abs() < 0.001);
        assert!((sizes[1] - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_resolve_tracks_auto() {
        let tracks = vec![GridTrackSize::Auto, GridTrackSize::Fr(1.0)];
        let content_sizes = vec![80.0, 0.0];
        let sizes = resolve_tracks(&tracks, 400.0, 0.0, &content_sizes);
        assert!((sizes[0] - 80.0).abs() < 0.001);
        assert!((sizes[1] - 320.0).abs() < 0.001);
    }

    #[test]
    fn test_place_items_auto() {
        // 6 items, 3 columns → 2 rows
        let placements: Vec<Option<&GridPlacement>> = vec![None; 6];
        let result = place_items(&placements, 3);
        assert_eq!(result.len(), 6);

        // First row: items 0, 1, 2
        assert_eq!(result[0].col_start, 0);
        assert_eq!(result[0].row_start, 0);
        assert_eq!(result[1].col_start, 1);
        assert_eq!(result[1].row_start, 0);
        assert_eq!(result[2].col_start, 2);
        assert_eq!(result[2].row_start, 0);

        // Second row: items 3, 4, 5
        assert_eq!(result[3].col_start, 0);
        assert_eq!(result[3].row_start, 1);
        assert_eq!(result[4].col_start, 1);
        assert_eq!(result[4].row_start, 1);
        assert_eq!(result[5].col_start, 2);
        assert_eq!(result[5].row_start, 1);
    }

    #[test]
    fn test_place_items_explicit() {
        let gp = GridPlacement {
            column_start: Some(2),
            column_end: None,
            row_start: Some(1),
            row_end: None,
            column_span: None,
            row_span: None,
        };
        let placements: Vec<Option<&GridPlacement>> = vec![Some(&gp), None, None];
        let result = place_items(&placements, 3);
        assert_eq!(result.len(), 3);

        // Find the explicitly placed item (child_index 0)
        let explicit = result.iter().find(|p| p.child_index == 0).unwrap();
        assert_eq!(explicit.col_start, 1); // column 2 → 0-based index 1
        assert_eq!(explicit.row_start, 0); // row 1 → 0-based index 0
    }

    #[test]
    fn test_place_items_spanning() {
        let gp = GridPlacement {
            column_start: None,
            column_end: None,
            row_start: None,
            row_end: None,
            column_span: Some(2),
            row_span: None,
        };
        let placements: Vec<Option<&GridPlacement>> = vec![Some(&gp), None, None];
        let result = place_items(&placements, 3);

        let spanning = result.iter().find(|p| p.child_index == 0).unwrap();
        assert_eq!(spanning.col_start, 0);
        assert_eq!(spanning.col_end, 2); // spans 2 columns
    }

    #[test]
    fn test_span_width() {
        let widths = vec![100.0, 200.0, 150.0];
        assert!((span_width(0, 1, &widths, 10.0) - 100.0).abs() < 0.001);
        assert!((span_width(0, 2, &widths, 10.0) - 310.0).abs() < 0.001); // 100 + 10 + 200
        assert!((span_width(0, 3, &widths, 10.0) - 470.0).abs() < 0.001); // 100 + 10 + 200 + 10 + 150
    }

    #[test]
    fn test_column_x_offset() {
        let widths = vec![100.0, 200.0, 150.0];
        assert!((column_x_offset(0, &widths, 10.0) - 0.0).abs() < 0.001);
        assert!((column_x_offset(1, &widths, 10.0) - 110.0).abs() < 0.001);
        assert!((column_x_offset(2, &widths, 10.0) - 320.0).abs() < 0.001);
    }
}

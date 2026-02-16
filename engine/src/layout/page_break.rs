//! # Page Break Decisions
//!
//! Logic for deciding when and how to break content across pages.
//! This module encodes the rules that make Forme's page breaks
//! feel natural rather than mechanical.

/// Decide what to do when a node doesn't fit on the current page.
#[derive(Debug, Clone, PartialEq)]
pub enum BreakDecision {
    /// Place the entire node on the current page (it fits).
    Place,
    /// Move the entire node to the next page (unbreakable, or better aesthetics).
    MoveToNextPage,
    /// Split the node: place some content here, continue on the next page.
    Split {
        /// How many child items / lines fit on the current page.
        items_on_current_page: usize,
    },
}

/// Given the remaining space on a page and a list of child heights,
/// decide how to break.
pub fn decide_break(
    remaining_height: f64,
    child_heights: &[f64],
    is_breakable: bool,
    min_orphan_lines: usize,
    min_widow_lines: usize,
) -> BreakDecision {
    // Total height of all children
    let total: f64 = child_heights.iter().sum();

    // Easy case: everything fits
    if total <= remaining_height {
        return BreakDecision::Place;
    }

    // Unbreakable: force to next page
    if !is_breakable {
        return BreakDecision::MoveToNextPage;
    }

    // Find how many children fit
    let mut running = 0.0;
    let mut fit_count = 0;
    for &h in child_heights {
        if running + h > remaining_height {
            break;
        }
        running += h;
        fit_count += 1;
    }

    // Widow/orphan control
    let total_items = child_heights.len();

    // Would we leave too few items on the current page? (orphan)
    if fit_count < min_orphan_lines && fit_count < total_items {
        return BreakDecision::MoveToNextPage;
    }

    // Would we leave too few items on the next page? (widow)
    let remaining_items = total_items - fit_count;
    if remaining_items < min_widow_lines && remaining_items > 0 {
        // Pull some items back to avoid widows
        let adjusted = fit_count.saturating_sub(min_widow_lines - remaining_items);
        if adjusted == 0 {
            return BreakDecision::MoveToNextPage;
        }
        return BreakDecision::Split {
            items_on_current_page: adjusted,
        };
    }

    if fit_count == 0 {
        return BreakDecision::MoveToNextPage;
    }

    BreakDecision::Split {
        items_on_current_page: fit_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_fits() {
        let decision = decide_break(100.0, &[20.0, 30.0, 40.0], true, 2, 2);
        assert_eq!(decision, BreakDecision::Place);
    }

    #[test]
    fn unbreakable_moves() {
        let decision = decide_break(50.0, &[20.0, 30.0, 40.0], false, 2, 2);
        assert_eq!(decision, BreakDecision::MoveToNextPage);
    }

    #[test]
    fn split_at_right_point() {
        let decision = decide_break(55.0, &[20.0, 30.0, 40.0], true, 1, 1);
        assert_eq!(
            decision,
            BreakDecision::Split {
                items_on_current_page: 2,
            }
        );
    }

    #[test]
    fn orphan_control() {
        // Only 1 item would fit, but min_orphan is 2 → move everything
        let decision = decide_break(25.0, &[20.0, 30.0, 40.0], true, 2, 2);
        assert_eq!(decision, BreakDecision::MoveToNextPage);
    }

    #[test]
    fn widow_control() {
        // 3 of 4 fit, leaving 1 widow (min=2) → pull one back
        let decision = decide_break(70.0, &[20.0, 20.0, 20.0, 20.0], true, 2, 2);
        assert_eq!(
            decision,
            BreakDecision::Split {
                items_on_current_page: 2,
            }
        );
    }
}

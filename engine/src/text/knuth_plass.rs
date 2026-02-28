//! # Knuth-Plass Optimal Line Breaking
//!
//! Implements the Knuth-Plass algorithm for computing optimal line breaks.
//! Instead of greedily filling each line, this considers all possible break
//! points and minimizes a global penalty (demerits) across the entire paragraph.
//!
//! The result: more even word spacing, fewer rivers of whitespace, and better
//! hyphenation decisions. Critical for justified text.

use unicode_linebreak::BreakOpportunity;

/// An item in the Knuth-Plass item list.
#[derive(Debug, Clone)]
pub enum Item {
    /// A fixed-width content box (word fragment, character).
    Box {
        width: f64,
        /// Char range [start, end) in the original char array.
        char_start: usize,
        char_end: usize,
    },
    /// Stretchable/shrinkable space (typically a word space).
    Glue {
        width: f64,
        stretch: f64,
        shrink: f64,
        /// Char index this glue corresponds to in the original text.
        char_index: usize,
    },
    /// A potential break point with a cost.
    Penalty {
        width: f64,
        penalty: f64,
        flagged: bool,
        /// Char index where this penalty sits.
        char_index: usize,
    },
}

/// Configuration for the Knuth-Plass algorithm.
#[derive(Debug, Clone)]
pub struct Config {
    pub line_width: f64,
    /// How much lines are allowed to stretch/shrink. Higher = more tolerance.
    pub tolerance: f64,
    /// Penalty for hyphenating a word.
    pub hyphen_penalty: f64,
    /// Extra demerits for two consecutive hyphenated lines.
    pub double_hyphen_demerits: f64,
    /// Extra demerits for adjacent lines with very different tightness.
    pub fitness_demerits: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            line_width: 0.0,
            tolerance: 2.0,
            hyphen_penalty: 50.0,
            double_hyphen_demerits: 3000.0,
            fitness_demerits: 100.0,
        }
    }
}

/// The solution for one line: where it breaks and how much the glue adjusts.
#[derive(Debug, Clone)]
pub struct LineSolution {
    /// Index into the items array where this line ends (the break item).
    pub break_item: usize,
    /// How much glue was stretched (positive) or shrunk (negative).
    /// Range roughly -1.0 to tolerance.
    pub adjustment_ratio: f64,
    /// Whether this line ends with a hyphen.
    pub is_hyphenated: bool,
}

/// Fitness class for the Knuth-Plass algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FitnessClass {
    Tight = 0,
    Normal = 1,
    Loose = 2,
    VeryLoose = 3,
}

fn fitness_class(ratio: f64) -> FitnessClass {
    if ratio < -0.5 {
        FitnessClass::Tight
    } else if ratio <= 0.5 {
        FitnessClass::Normal
    } else if ratio <= 1.0 {
        FitnessClass::Loose
    } else {
        FitnessClass::VeryLoose
    }
}

/// An active breakpoint in the DP.
#[derive(Debug, Clone)]
struct Breakpoint {
    /// Index in the items array where this break occurs.
    item_index: usize,
    /// Line number (0-based) ending at this break.
    line: usize,
    /// Fitness class of the line ending at this break.
    fitness: FitnessClass,
    /// Total width of items from the start of the paragraph to this break.
    total_width: f64,
    /// Total stretch available from the start to this break.
    total_stretch: f64,
    /// Total shrink available from the start to this break.
    total_shrink: f64,
    /// Total demerits accumulated up to this break.
    total_demerits: f64,
    /// Index of the previous breakpoint in the chain (for backtracking).
    prev: Option<usize>,
    /// Whether this break was a hyphenation.
    is_hyphenated: bool,
}

/// Build items from a plain-text char array with pre-computed widths.
///
/// This converts the character-level data into the box/glue/penalty sequence
/// that the Knuth-Plass algorithm operates on.
pub fn build_items(
    chars: &[char],
    char_widths: &[f64],
    hyphen_width: f64,
    hyphens: crate::style::Hyphens,
    break_opps: &[Option<BreakOpportunity>],
    lang: Option<&str>,
) -> Vec<Item> {
    let mut items = Vec::new();
    let mut box_start = 0;
    let mut box_width = 0.0;
    let _space_width = if !char_widths.is_empty() {
        // Estimate space width from char_widths — find the first space
        chars
            .iter()
            .zip(char_widths.iter())
            .find(|(c, _)| **c == ' ')
            .map(|(_, w)| *w)
            .unwrap_or(char_widths[0])
    } else {
        0.0
    };

    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];

        // Skip soft hyphens (zero-width when not breaking)
        if ch == '\u{00AD}' {
            // Soft hyphen: emit current box, add penalty for possible break
            if box_width > 0.0 {
                items.push(Item::Box {
                    width: box_width,
                    char_start: box_start,
                    char_end: i,
                });
                box_width = 0.0;
            }
            if hyphens != crate::style::Hyphens::None {
                items.push(Item::Penalty {
                    width: hyphen_width,
                    penalty: 50.0,
                    flagged: true,
                    char_index: i,
                });
            }
            i += 1;
            box_start = i;
            continue;
        }

        // Skip newline/CR (handled as mandatory breaks)
        if ch == '\n' || ch == '\r' || ch == '\u{2028}' || ch == '\u{2029}' {
            if box_width > 0.0 {
                items.push(Item::Box {
                    width: box_width,
                    char_start: box_start,
                    char_end: i,
                });
                box_width = 0.0;
            }
            // Mandatory break: penalty of -infinity
            items.push(Item::Penalty {
                width: 0.0,
                penalty: f64::NEG_INFINITY,
                flagged: false,
                char_index: i,
            });
            i += 1;
            box_start = i;
            continue;
        }

        // Check for UAX#14 break opportunity *before* this char
        if i > 0 {
            if let Some(opp) = break_opps[i] {
                match opp {
                    BreakOpportunity::Mandatory => {
                        if box_width > 0.0 {
                            items.push(Item::Box {
                                width: box_width,
                                char_start: box_start,
                                char_end: i,
                            });
                            box_width = 0.0;
                        }
                        items.push(Item::Penalty {
                            width: 0.0,
                            penalty: f64::NEG_INFINITY,
                            flagged: false,
                            char_index: i,
                        });
                        box_start = i;
                    }
                    BreakOpportunity::Allowed => {
                        // If the previous char was a space, we already emitted glue
                        // Only add a zero-width penalty for non-space allowed breaks
                        // (e.g., CJK inter-character breaks, after hyphens)
                        let prev_is_space = i > 0 && chars[i - 1] == ' ';
                        if !prev_is_space {
                            if box_width > 0.0 {
                                items.push(Item::Box {
                                    width: box_width,
                                    char_start: box_start,
                                    char_end: i,
                                });
                                box_width = 0.0;
                                box_start = i;
                            }
                            items.push(Item::Penalty {
                                width: 0.0,
                                penalty: 0.0,
                                flagged: false,
                                char_index: i,
                            });
                        }
                    }
                }
            }
        }

        // Space → emit box + glue
        if ch == ' ' || ch == '\t' {
            if box_width > 0.0 {
                items.push(Item::Box {
                    width: box_width,
                    char_start: box_start,
                    char_end: i,
                });
                box_width = 0.0;
            }
            let w = char_widths[i];
            items.push(Item::Glue {
                width: w,
                stretch: w * 0.5,
                shrink: w * 0.33,
                char_index: i,
            });
            i += 1;
            box_start = i;
            continue;
        }

        // Regular character → accumulate into current box
        box_width += char_widths[i];
        i += 1;
    }

    // Flush trailing box
    if box_width > 0.0 {
        items.push(Item::Box {
            width: box_width,
            char_start: box_start,
            char_end: chars.len(),
        });
    }

    // Add hyphenation penalties if hyphens == Auto
    if hyphens == crate::style::Hyphens::Auto {
        items = insert_hyphenation_penalties(items, chars, char_widths, hyphen_width, lang);
    }

    // Final forced break (the paragraph must end)
    items.push(Item::Glue {
        width: 0.0,
        stretch: 1e6,
        shrink: 0.0,
        char_index: chars.len(),
    });
    items.push(Item::Penalty {
        width: 0.0,
        penalty: f64::NEG_INFINITY,
        flagged: false,
        char_index: chars.len(),
    });

    items
}

/// Insert hyphenation penalties into the item list at syllable boundaries.
fn insert_hyphenation_penalties(
    items: Vec<Item>,
    chars: &[char],
    char_widths: &[f64],
    hyphen_width: f64,
    lang: Option<&str>,
) -> Vec<Item> {
    let hypher_lang = match super::resolve_hypher_lang(lang) {
        Some(l) => l,
        None => return items,
    };

    let mut result = Vec::with_capacity(items.len());

    for item in items {
        match &item {
            Item::Box {
                char_start,
                char_end,
                ..
            } => {
                let start = *char_start;
                let end = *char_end;
                // Only hyphenate boxes that are actual words (alpha chars)
                let word: String = chars[start..end].iter().collect();
                if word.len() < 4
                    || !word.chars().all(|c| c.is_alphabetic())
                    || word.chars().all(|c| c.is_whitespace())
                {
                    result.push(item);
                    continue;
                }

                let syllables: Vec<&str> = hypher::hyphenate(&word, hypher_lang).collect();
                if syllables.len() < 2 {
                    result.push(item);
                    continue;
                }

                // Split this box into sub-boxes with penalty breaks between them
                let mut offset = start;
                for (si, syllable) in syllables.iter().enumerate() {
                    let syl_len = syllable.chars().count();
                    let syl_end = offset + syl_len;
                    let syl_width: f64 = char_widths[offset..syl_end].iter().sum();

                    result.push(Item::Box {
                        width: syl_width,
                        char_start: offset,
                        char_end: syl_end,
                    });

                    if si < syllables.len() - 1 {
                        result.push(Item::Penalty {
                            width: hyphen_width,
                            penalty: 50.0,
                            flagged: true,
                            char_index: syl_end,
                        });
                    }

                    offset = syl_end;
                }
            }
            _ => {
                result.push(item);
            }
        }
    }

    result
}

/// Find optimal break points using the Knuth-Plass algorithm.
///
/// Returns `None` if no feasible solution exists (text can't fit at all).
pub fn find_breaks(items: &[Item], config: &Config) -> Option<Vec<LineSolution>> {
    let mut breakpoints: Vec<Breakpoint> = Vec::new();
    let mut active: Vec<usize> = Vec::new();

    // Start with a breakpoint at the beginning
    breakpoints.push(Breakpoint {
        item_index: 0,
        line: 0,
        fitness: FitnessClass::Normal,
        total_width: 0.0,
        total_stretch: 0.0,
        total_shrink: 0.0,
        total_demerits: 0.0,
        prev: None,
        is_hyphenated: false,
    });
    active.push(0);

    // Running totals up to current position
    let mut total_width = 0.0;
    let mut total_stretch = 0.0;
    let mut total_shrink = 0.0;

    for (i, item) in items.iter().enumerate() {
        // Check if this is a feasible break point BEFORE updating totals for glue.
        // For glue breaks, the break is BEFORE the glue (glue is consumed).
        // For penalty breaks, break is AT the penalty.
        let is_break = match item {
            Item::Penalty { penalty, .. } => *penalty < f64::INFINITY,
            Item::Glue { .. } => {
                // Can break before glue if preceded by a box
                i > 0 && matches!(items[i - 1], Item::Box { .. })
            }
            _ => false,
        };

        if is_break {
            // For each active breakpoint, compute the adjustment ratio
            let mut new_active = Vec::new();
            let mut best_by_fitness: [Option<(f64, usize)>; 4] = [None; 4];

            let mut deactivate = Vec::new();

            for &a_idx in &active {
                let a = &breakpoints[a_idx];

                // Width of items on this line (from break a to break i).
                // total_width at this point does NOT include the current item yet.
                let line_width = total_width - a.total_width;
                let line_stretch = total_stretch - a.total_stretch;
                let line_shrink = total_shrink - a.total_shrink;

                // Add penalty width if breaking at a penalty
                let penalty_width = match item {
                    Item::Penalty { width, .. } => *width,
                    _ => 0.0,
                };

                let actual_width = line_width + penalty_width;
                let target = config.line_width;

                let ratio = if actual_width < target {
                    // Need to stretch
                    if line_stretch > 0.0 {
                        (target - actual_width) / line_stretch
                    } else {
                        f64::INFINITY
                    }
                } else if actual_width > target {
                    // Need to shrink
                    if line_shrink > 0.0 {
                        (target - actual_width) / line_shrink
                    } else {
                        f64::INFINITY
                    }
                } else {
                    0.0 // Perfect fit
                };

                // Check if this break is feasible
                if ratio < -1.0 || ratio > config.tolerance {
                    // If ratio < -1, we've compressed as much as possible and it still overflows.
                    // If this is because ratio < -1 (overfull), deactivate this breakpoint.
                    if ratio < -1.0 {
                        deactivate.push(a_idx);
                    }
                    continue;
                }

                // Compute demerits for this break
                let penalty_val = match item {
                    Item::Penalty { penalty, .. } => *penalty,
                    _ => 0.0,
                };
                let flagged = match item {
                    Item::Penalty { flagged, .. } => *flagged,
                    _ => false,
                };

                let badness = 100.0 * ratio.abs().powi(3);
                let demerits = if penalty_val >= 0.0 {
                    (1.0 + badness + penalty_val).powi(2)
                } else if penalty_val > f64::NEG_INFINITY {
                    (1.0 + badness).powi(2) - penalty_val.powi(2)
                } else {
                    (1.0 + badness).powi(2)
                };

                // Extra demerits for consecutive hyphenated lines
                let demerits = if flagged && a.is_hyphenated {
                    demerits + config.double_hyphen_demerits
                } else {
                    demerits
                };

                // Extra demerits for fitness class mismatch
                let fc = fitness_class(ratio);
                let demerits = if (fc as i32 - a.fitness as i32).unsigned_abs() > 1 {
                    demerits + config.fitness_demerits
                } else {
                    demerits
                };

                let total = a.total_demerits + demerits;

                let slot = fc as usize;
                if best_by_fitness[slot].is_none() || total < best_by_fitness[slot].unwrap().0 {
                    best_by_fitness[slot] = Some((total, a_idx));
                }
            }

            // Deactivate overfull breakpoints
            for d in &deactivate {
                active.retain(|x| x != d);
            }

            // Compute "after break" totals: for glue breaks, include the glue;
            // for penalty breaks, just use current totals (penalty is zero-width in totals).
            let (bp_tw, bp_ts, bp_tk) = match item {
                Item::Glue {
                    width,
                    stretch,
                    shrink,
                    ..
                } => (
                    total_width + width,
                    total_stretch + stretch,
                    total_shrink + shrink,
                ),
                _ => (total_width, total_stretch, total_shrink),
            };

            // Create new breakpoints for the best of each fitness class
            for (fc_idx, best) in best_by_fitness.iter().enumerate() {
                if let Some((total_demerits, prev_idx)) = best {
                    let is_hyph = matches!(item, Item::Penalty { flagged: true, .. });
                    let bp_idx = breakpoints.len();
                    breakpoints.push(Breakpoint {
                        item_index: i,
                        line: breakpoints[*prev_idx].line + 1,
                        fitness: match fc_idx {
                            0 => FitnessClass::Tight,
                            1 => FitnessClass::Normal,
                            2 => FitnessClass::Loose,
                            _ => FitnessClass::VeryLoose,
                        },
                        total_width: bp_tw,
                        total_stretch: bp_ts,
                        total_shrink: bp_tk,
                        total_demerits: *total_demerits,
                        prev: Some(*prev_idx),
                        is_hyphenated: is_hyph,
                    });
                    new_active.push(bp_idx);
                }
            }

            active.extend(new_active);

            // If no active breakpoints remain, we're in trouble — give up
            if active.is_empty() {
                return None;
            }
        }

        // Update running totals AFTER processing breaks
        match item {
            Item::Box { width, .. } => {
                total_width += width;
            }
            Item::Glue {
                width,
                stretch,
                shrink,
                ..
            } => {
                total_width += width;
                total_stretch += stretch;
                total_shrink += shrink;
            }
            Item::Penalty { .. } => {}
        }
    }

    // Find the best final breakpoint — must be at the last item (the forced
    // paragraph-ending break). The initial dummy breakpoint at item 0 may still
    // be active but is not a valid solution endpoint.
    let last_item = items.len() - 1;
    let best_final = active
        .iter()
        .filter(|&&idx| breakpoints[idx].item_index == last_item)
        .min_by(|&&a, &&b| {
            breakpoints[a]
                .total_demerits
                .partial_cmp(&breakpoints[b].total_demerits)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()?;

    // Backtrack to build the solution
    let mut solutions = Vec::new();
    let mut current = Some(best_final);

    while let Some(idx) = current {
        let bp = &breakpoints[idx];
        if let Some(prev_idx) = bp.prev {
            // This is a real break (not the initial dummy)
            let prev_bp = &breakpoints[prev_idx];

            // Compute adjustment ratio for this line.
            // For glue breaks, bp.total_* includes the break glue (which is
            // consumed, not on the line), so subtract it back out.
            let (glue_w, glue_st, glue_sh) = match &items[bp.item_index] {
                Item::Glue {
                    width,
                    stretch,
                    shrink,
                    ..
                } => (*width, *stretch, *shrink),
                _ => (0.0, 0.0, 0.0),
            };
            let line_w = bp.total_width - prev_bp.total_width - glue_w;
            let line_stretch = bp.total_stretch - prev_bp.total_stretch - glue_st;
            let line_shrink = bp.total_shrink - prev_bp.total_shrink - glue_sh;

            let penalty_w = match &items[bp.item_index] {
                Item::Penalty { width, .. } => *width,
                _ => 0.0,
            };

            let actual = line_w + penalty_w;
            let target = config.line_width;
            let ratio = if actual < target && line_stretch > 0.0 {
                (target - actual) / line_stretch
            } else if actual > target && line_shrink > 0.0 {
                (target - actual) / line_shrink
            } else {
                0.0
            };

            solutions.push(LineSolution {
                break_item: bp.item_index,
                adjustment_ratio: ratio,
                is_hyphenated: bp.is_hyphenated,
            });
        }
        current = bp.prev;
    }

    solutions.reverse();
    Some(solutions)
}

/// Reconstruct broken lines from KP solutions, with justified spacing.
///
/// For justified text, spaces are stretched/shrunk according to the
/// adjustment ratio. The last line is always left-aligned.
pub fn reconstruct_lines(
    solutions: &[LineSolution],
    items: &[Item],
    chars: &[char],
    char_widths: &[f64],
    line_width: f64,
    justify: bool,
) -> Vec<super::BrokenLine> {
    let mut lines = Vec::new();
    let mut item_start = 0;

    for (sol_idx, sol) in solutions.iter().enumerate() {
        let is_last_line = sol_idx == solutions.len() - 1;
        let apply_justify = justify && !is_last_line;

        let mut line_chars = Vec::new();
        let mut line_positions = Vec::new();
        let mut x = 0.0;

        for (item_idx, item) in items
            .iter()
            .enumerate()
            .take(sol.break_item + 1)
            .skip(item_start)
        {
            match item {
                Item::Box {
                    char_start,
                    char_end,
                    ..
                } => {
                    for ci in *char_start..*char_end {
                        if chars[ci] == '\u{00AD}' {
                            continue;
                        }
                        line_chars.push(chars[ci]);
                        line_positions.push(x);
                        x += char_widths[ci];
                    }
                }
                Item::Glue {
                    width,
                    stretch,
                    shrink,
                    char_index,
                } => {
                    // Skip trailing glue at the break point
                    if item_idx == sol.break_item {
                        continue;
                    }
                    let adjusted = if apply_justify {
                        if sol.adjustment_ratio >= 0.0 {
                            width + sol.adjustment_ratio * stretch
                        } else {
                            width + sol.adjustment_ratio * shrink
                        }
                    } else {
                        *width
                    };
                    if *char_index < chars.len() {
                        line_chars.push(chars[*char_index]);
                        line_positions.push(x);
                    }
                    x += adjusted;
                }
                Item::Penalty { flagged, width, .. } => {
                    // If this is the break point and it's a hyphenation, add a hyphen
                    if item_idx == sol.break_item && *flagged {
                        line_chars.push('-');
                        line_positions.push(x);
                        x += width;
                    }
                }
            }
        }

        // Trim trailing spaces from width
        let mut effective_width = x;
        let mut trim_idx = line_chars.len();
        while trim_idx > 0 && line_chars[trim_idx - 1] == ' ' {
            trim_idx -= 1;
            if trim_idx < line_positions.len() {
                let pos = line_positions[trim_idx];
                effective_width = pos;
            }
        }

        // For justified text (non-last line), width should be close to target
        if apply_justify {
            effective_width = effective_width.min(line_width);
        }

        lines.push(super::BrokenLine {
            text: line_chars.iter().collect(),
            chars: line_chars,
            char_positions: line_positions,
            width: effective_width,
        });

        // Next line starts after the break item
        // Skip glue after the break
        item_start = sol.break_item + 1;
        while item_start < items.len() && matches!(items[item_start], Item::Glue { .. }) {
            item_start += 1;
        }
    }

    lines
}

/// Build items from multi-style (StyledChar) text.
pub fn build_items_styled(
    chars: &[super::StyledChar],
    char_widths: &[f64],
    hyphen_width: f64,
    hyphens: crate::style::Hyphens,
    break_opps: &[Option<BreakOpportunity>],
    lang: Option<&str>,
) -> Vec<Item> {
    let plain_chars: Vec<char> = chars.iter().map(|sc| sc.ch).collect();
    build_items(
        &plain_chars,
        char_widths,
        hyphen_width,
        hyphens,
        break_opps,
        lang,
    )
}

/// Reconstruct run-broken lines from KP solutions, with justified spacing.
pub fn reconstruct_run_lines(
    solutions: &[LineSolution],
    items: &[Item],
    chars: &[super::StyledChar],
    char_widths: &[f64],
    line_width: f64,
    justify: bool,
) -> Vec<super::RunBrokenLine> {
    let mut lines = Vec::new();
    let mut item_start = 0;

    for (sol_idx, sol) in solutions.iter().enumerate() {
        let is_last_line = sol_idx == solutions.len() - 1;
        let apply_justify = justify && !is_last_line;

        let mut line_chars: Vec<super::StyledChar> = Vec::new();
        let mut line_positions = Vec::new();
        let mut x = 0.0;

        for (item_idx, item) in items
            .iter()
            .enumerate()
            .take(sol.break_item + 1)
            .skip(item_start)
        {
            match item {
                Item::Box {
                    char_start,
                    char_end,
                    ..
                } => {
                    for ci in *char_start..*char_end {
                        if chars[ci].ch == '\u{00AD}' {
                            continue;
                        }
                        line_chars.push(chars[ci].clone());
                        line_positions.push(x);
                        x += char_widths[ci];
                    }
                }
                Item::Glue {
                    width,
                    stretch,
                    shrink,
                    char_index,
                } => {
                    if item_idx == sol.break_item {
                        continue;
                    }
                    let adjusted = if apply_justify {
                        if sol.adjustment_ratio >= 0.0 {
                            width + sol.adjustment_ratio * stretch
                        } else {
                            width + sol.adjustment_ratio * shrink
                        }
                    } else {
                        *width
                    };
                    if *char_index < chars.len() {
                        line_chars.push(chars[*char_index].clone());
                        line_positions.push(x);
                    }
                    x += adjusted;
                }
                Item::Penalty {
                    flagged,
                    char_index,
                    width,
                    ..
                } => {
                    if item_idx == sol.break_item && *flagged {
                        // Add hyphen with the style of the preceding char
                        let style_ref = if *char_index > 0 {
                            &chars[char_index - 1]
                        } else {
                            &chars[0]
                        };
                        let mut hyphen_sc = style_ref.clone();
                        hyphen_sc.ch = '-';
                        line_chars.push(hyphen_sc);
                        line_positions.push(x);
                        x += width;
                    }
                }
            }
        }

        // Trim trailing spaces
        let mut effective_width = x;
        let mut trim_idx = line_chars.len();
        while trim_idx > 0 && line_chars[trim_idx - 1].ch == ' ' {
            trim_idx -= 1;
            if trim_idx < line_positions.len() {
                effective_width = line_positions[trim_idx];
            }
        }

        if apply_justify {
            effective_width = effective_width.min(line_width);
        }

        lines.push(super::RunBrokenLine {
            chars: line_chars,
            char_positions: line_positions,
            width: effective_width,
        });

        item_start = sol.break_item + 1;
        while item_start < items.len() && matches!(items[item_start], Item::Glue { .. }) {
            item_start += 1;
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_items(text: &str, char_width: f64) -> (Vec<char>, Vec<f64>) {
        let chars: Vec<char> = text.chars().collect();
        let widths = vec![char_width; chars.len()];
        (chars, widths)
    }

    #[test]
    fn test_build_items_simple() {
        let (chars, widths) = simple_items("Hello World", 10.0);
        let break_opps = super::super::compute_break_opportunities("Hello World");
        let items = build_items(
            &chars,
            &widths,
            5.0,
            crate::style::Hyphens::Manual,
            &break_opps,
            None,
        );
        // Should have: Box("Hello") + Glue(" ") + Box("World") + final glue + final penalty
        let boxes: Vec<_> = items
            .iter()
            .filter(|i| matches!(i, Item::Box { .. }))
            .collect();
        assert_eq!(boxes.len(), 2, "Should have 2 boxes (Hello, World)");
    }

    #[test]
    fn test_find_breaks_single_line() {
        let (chars, widths) = simple_items("Hello World", 10.0);
        let break_opps = super::super::compute_break_opportunities("Hello World");
        let items = build_items(
            &chars,
            &widths,
            5.0,
            crate::style::Hyphens::Manual,
            &break_opps,
            None,
        );
        let config = Config {
            line_width: 200.0, // wide enough for everything
            ..Default::default()
        };
        let solutions = find_breaks(&items, &config).expect("Should find solution");
        assert_eq!(solutions.len(), 1, "Should be 1 line");
    }

    #[test]
    fn test_find_breaks_multi_line() {
        let (chars, widths) = simple_items("aa bb cc dd ee", 10.0);
        let break_opps = super::super::compute_break_opportunities("aa bb cc dd ee");
        let items = build_items(
            &chars,
            &widths,
            5.0,
            crate::style::Hyphens::Manual,
            &break_opps,
            None,
        );
        let config = Config {
            line_width: 55.0, // "xx yy" = 50 wide, ratio 1.0 with 5 stretch
            ..Default::default()
        };
        let solutions = find_breaks(&items, &config).expect("Should find solution");
        assert!(
            solutions.len() >= 2,
            "Should need multiple lines, got {}",
            solutions.len()
        );
    }

    #[test]
    fn test_adjustment_ratio() {
        let (chars, widths) = simple_items("Hello World", 10.0);
        let break_opps = super::super::compute_break_opportunities("Hello World");
        let items = build_items(
            &chars,
            &widths,
            5.0,
            crate::style::Hyphens::Manual,
            &break_opps,
            None,
        );
        let config = Config {
            line_width: 200.0,
            ..Default::default()
        };
        let solutions = find_breaks(&items, &config).expect("Should find solution");
        // Single line — last line has ratio 0 (or close)
        for sol in &solutions {
            assert!(
                sol.adjustment_ratio.is_finite(),
                "Adjustment ratio should be finite"
            );
        }
    }

    #[test]
    fn test_justify_spacing() {
        // Justified non-final lines should fill to line_width
        let (chars, widths) = simple_items("aa bb cc dd ee", 10.0);
        let break_opps = super::super::compute_break_opportunities("aa bb cc dd ee");
        let items = build_items(
            &chars,
            &widths,
            5.0,
            crate::style::Hyphens::Manual,
            &break_opps,
            None,
        );
        let config = Config {
            line_width: 55.0,
            ..Default::default()
        };
        let solutions = find_breaks(&items, &config).expect("Should find solution");
        let lines = reconstruct_lines(&solutions, &items, &chars, &widths, 55.0, true);
        assert!(lines.len() >= 2);
        // Non-final lines should have width close to line_width
        for (i, line) in lines.iter().enumerate() {
            if i < lines.len() - 1 {
                assert!(
                    (line.width - 55.0).abs() < 1.0,
                    "Justified line {} width should be ~55, got {}",
                    i,
                    line.width
                );
            }
        }
    }

    #[test]
    fn test_hyphenation_penalty() {
        let (chars, widths) = simple_items("extraordinary", 10.0);
        let break_opps = super::super::compute_break_opportunities("extraordinary");
        let items = build_items(
            &chars,
            &widths,
            5.0,
            crate::style::Hyphens::Auto,
            &break_opps,
            None,
        );
        let penalties: Vec<_> = items
            .iter()
            .filter(|i| matches!(i, Item::Penalty { flagged: true, .. }))
            .collect();
        assert!(
            !penalties.is_empty(),
            "Should have hyphenation penalties for 'extraordinary'"
        );
    }
}

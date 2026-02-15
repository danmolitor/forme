//! # TrueType Font Subsetter
//!
//! Strips a TrueType font to only the glyphs actually used in the document.
//! This dramatically reduces PDF size — a typical font is 50-200KB but a
//! subset with ~100 glyphs is usually 5-15KB.
//!
//! The subsetter rebuilds a valid TrueType file with remapped glyph IDs
//! (contiguous starting from 0). This is important because PDF CIDFont
//! width arrays and content stream glyph references must use the new IDs.
//!
//! ## Approach
//!
//! 1. Collect all needed glyphs (used glyphs + composite glyph dependencies)
//! 2. Remap old GIDs to new contiguous GIDs
//! 3. Rebuild required TrueType tables (glyf, loca, hmtx, cmap, etc.)
//! 4. Write a valid TrueType file with correct checksums and alignment

use std::collections::{BTreeSet, HashMap};

/// Result of subsetting a font.
pub struct SubsetResult {
    /// The subset TrueType file bytes.
    pub ttf_data: Vec<u8>,
    /// Maps original glyph IDs to new contiguous glyph IDs.
    pub gid_remap: HashMap<u16, u16>,
}

/// Subset a TrueType font to only include the given glyph IDs.
pub fn subset_ttf(ttf_data: &[u8], used_gids: &std::collections::HashSet<u16>) -> Result<SubsetResult, String> {
    let face = ttf_parser::Face::parse(ttf_data, 0)
        .map_err(|e| format!("Failed to parse TTF: {:?}", e))?;

    // Always include glyph 0 (.notdef)
    let mut needed_gids: BTreeSet<u16> = BTreeSet::new();
    needed_gids.insert(0);
    for &gid in used_gids {
        needed_gids.insert(gid);
    }

    // Resolve composite glyph dependencies
    let raw_glyf = find_table(ttf_data, b"glyf")
        .ok_or("Missing glyf table")?;
    let raw_loca = find_table(ttf_data, b"loca")
        .ok_or("Missing loca table")?;
    let head = find_table(ttf_data, b"head")
        .ok_or("Missing head table")?;

    let num_glyphs = face.number_of_glyphs();
    let loca_format = read_i16(head, 50); // indexToLocFormat at offset 50
    let loca_offsets = parse_loca(raw_loca, loca_format, num_glyphs)?;

    // Recursively collect composite glyph component GIDs
    let initial_gids: Vec<u16> = needed_gids.iter().copied().collect();
    for gid in initial_gids {
        collect_composite_deps(raw_glyf, &loca_offsets, gid, &mut needed_gids);
    }

    // Build remap: old GID → new contiguous GID
    let mut gid_remap: HashMap<u16, u16> = HashMap::new();
    for (new_gid, &old_gid) in needed_gids.iter().enumerate() {
        gid_remap.insert(old_gid, new_gid as u16);
    }

    let new_num_glyphs = needed_gids.len() as u16;

    // Rebuild glyf table with remapped composite references
    let (new_glyf, new_loca_offsets) = rebuild_glyf(
        raw_glyf, &loca_offsets, &needed_gids, &gid_remap,
    );

    // Determine loca format based on glyf size
    let new_loca_format: i16 = if new_glyf.len() > 0x1FFFE { 1 } else { 0 };
    let new_loca = build_loca(&new_loca_offsets, new_loca_format);

    // Rebuild hmtx (horizontal metrics)
    let raw_hmtx = find_table(ttf_data, b"hmtx")
        .ok_or("Missing hmtx table")?;
    let raw_hhea = find_table(ttf_data, b"hhea")
        .ok_or("Missing hhea table")?;
    let num_h_metrics = read_u16(raw_hhea, 34) as usize;
    let new_hmtx = rebuild_hmtx(raw_hmtx, &needed_gids, num_h_metrics);

    // Build minimal cmap (Format 4)
    // We need the original char→gid mapping — invert through the face
    let mut char_to_new_gid: Vec<(u16, u16)> = Vec::new();
    for &old_gid in &needed_gids {
        if old_gid == 0 { continue; }
        // Search for Unicode codepoint that maps to this GID
        // This is O(n) per glyph but subset sizes are small
        for code in 0u32..=0xFFFF {
            if let Some(ch) = char::from_u32(code) {
                if let Some(gid) = face.glyph_index(ch) {
                    if gid.0 == old_gid {
                        if let Some(&new_gid) = gid_remap.get(&old_gid) {
                            char_to_new_gid.push((code as u16, new_gid));
                        }
                        break;
                    }
                }
            }
        }
    }
    let new_cmap = build_cmap_format4(&char_to_new_gid);

    // Copy or rebuild remaining required tables
    let new_head = rebuild_head(head, new_loca_format);

    let raw_hhea_data = raw_hhea.to_vec();
    let new_hhea = rebuild_hhea(&raw_hhea_data, new_num_glyphs);

    let new_maxp = build_maxp(new_num_glyphs);
    let new_post = build_post_format3();

    // Copy name table verbatim if present (or build minimal)
    let new_name = find_table(ttf_data, b"name")
        .map(|t| t.to_vec())
        .unwrap_or_else(|| build_minimal_name(&face));

    // Copy OS/2 table verbatim if present
    let new_os2 = find_table(ttf_data, b"OS/2")
        .map(|t| t.to_vec());

    // Copy hinting tables verbatim if present
    let cvt_data = find_table(ttf_data, b"cvt ").map(|t| t.to_vec());
    let fpgm_data = find_table(ttf_data, b"fpgm").map(|t| t.to_vec());
    let prep_data = find_table(ttf_data, b"prep").map(|t| t.to_vec());

    // Assemble the final TrueType file
    let mut tables: Vec<(u32, Vec<u8>)> = Vec::new();
    tables.push((tag_u32(b"cmap"), new_cmap));
    if let Some(cvt) = cvt_data { tables.push((tag_u32(b"cvt "), cvt)); }
    if let Some(fpgm) = fpgm_data { tables.push((tag_u32(b"fpgm"), fpgm)); }
    tables.push((tag_u32(b"glyf"), new_glyf));
    tables.push((tag_u32(b"head"), new_head));
    tables.push((tag_u32(b"hhea"), new_hhea));
    tables.push((tag_u32(b"hmtx"), new_hmtx));
    tables.push((tag_u32(b"loca"), new_loca));
    tables.push((tag_u32(b"maxp"), new_maxp));
    tables.push((tag_u32(b"name"), new_name));
    if let Some(os2) = new_os2 { tables.push((tag_u32(b"OS/2"), os2)); }
    tables.push((tag_u32(b"post"), new_post));
    if let Some(prep) = prep_data { tables.push((tag_u32(b"prep"), prep)); }

    // Sort tables by tag (required by TrueType spec for binary search)
    tables.sort_by_key(|(tag, _)| *tag);

    let output = write_ttf_file(&mut tables);

    Ok(SubsetResult {
        ttf_data: output,
        gid_remap,
    })
}

// ─── Table Locating ─────────────────────────────────────────────

fn find_table<'a>(data: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    if data.len() < 12 { return None; }
    let num_tables = read_u16(data, 4) as usize;
    for i in 0..num_tables {
        let offset = 12 + i * 16;
        if offset + 16 > data.len() { break; }
        if &data[offset..offset + 4] == tag {
            let table_offset = read_u32(data, offset + 8) as usize;
            let table_length = read_u32(data, offset + 12) as usize;
            if table_offset + table_length <= data.len() {
                return Some(&data[table_offset..table_offset + table_length]);
            }
        }
    }
    None
}

// ─── Loca Table Parsing ─────────────────────────────────────────

fn parse_loca(data: &[u8], format: i16, num_glyphs: u16) -> Result<Vec<u32>, String> {
    let count = num_glyphs as usize + 1; // loca has numGlyphs + 1 entries
    let mut offsets = Vec::with_capacity(count);

    if format == 0 {
        // Short format: offsets are u16, multiply by 2
        for i in 0..count {
            let pos = i * 2;
            if pos + 2 > data.len() {
                offsets.push(*offsets.last().unwrap_or(&0));
            } else {
                offsets.push(read_u16(data, pos) as u32 * 2);
            }
        }
    } else {
        // Long format: offsets are u32
        for i in 0..count {
            let pos = i * 4;
            if pos + 4 > data.len() {
                offsets.push(*offsets.last().unwrap_or(&0));
            } else {
                offsets.push(read_u32(data, pos));
            }
        }
    }

    Ok(offsets)
}

// ─── Composite Glyph Dependency Collection ──────────────────────

fn collect_composite_deps(
    glyf: &[u8],
    loca_offsets: &[u32],
    gid: u16,
    needed: &mut BTreeSet<u16>,
) {
    let idx = gid as usize;
    if idx + 1 >= loca_offsets.len() { return; }

    let start = loca_offsets[idx] as usize;
    let end = loca_offsets[idx + 1] as usize;
    if start >= end || start + 10 > glyf.len() { return; }

    let num_contours = read_i16(glyf, start);
    if num_contours >= 0 { return; } // Simple glyph, no deps

    // Composite glyph — walk component records
    let mut pos = start + 10; // skip header (numContours + bbox)

    loop {
        if pos + 4 > glyf.len() { break; }
        let flags = read_u16(glyf, pos);
        let component_gid = read_u16(glyf, pos + 2);
        pos += 4;

        if needed.insert(component_gid) {
            // Recursively collect deps for newly discovered component
            collect_composite_deps(glyf, loca_offsets, component_gid, needed);
        }

        // Determine how many bytes of arguments follow
        if flags & 0x0001 != 0 {
            // ARG_1_AND_2_ARE_WORDS: 2 × i16
            pos += 4;
        } else {
            // 2 × i8
            pos += 2;
        }

        // Transform matrix components
        if flags & 0x0008 != 0 {
            // WE_HAVE_A_SCALE: 1 × F2Dot14
            pos += 2;
        } else if flags & 0x0040 != 0 {
            // WE_HAVE_AN_X_AND_Y_SCALE: 2 × F2Dot14
            pos += 4;
        } else if flags & 0x0080 != 0 {
            // WE_HAVE_A_TWO_BY_TWO: 4 × F2Dot14
            pos += 8;
        }

        if flags & 0x0020 == 0 {
            // MORE_COMPONENTS flag not set — done
            break;
        }
    }
}

// ─── Table Rebuilding ───────────────────────────────────────────

fn rebuild_glyf(
    glyf: &[u8],
    loca_offsets: &[u32],
    needed_gids: &BTreeSet<u16>,
    gid_remap: &HashMap<u16, u16>,
) -> (Vec<u8>, Vec<u32>) {
    let mut new_glyf: Vec<u8> = Vec::new();
    let mut new_offsets: Vec<u32> = Vec::new();

    for &old_gid in needed_gids {
        new_offsets.push(new_glyf.len() as u32);

        let idx = old_gid as usize;
        if idx + 1 >= loca_offsets.len() {
            continue;
        }

        let start = loca_offsets[idx] as usize;
        let end = loca_offsets[idx + 1] as usize;
        if start >= end || start >= glyf.len() {
            // Empty glyph
            continue;
        }

        let glyph_data = &glyf[start..end.min(glyf.len())];
        let mut new_glyph = glyph_data.to_vec();

        // If composite, rewrite component GID references
        if glyph_data.len() >= 2 {
            let num_contours = read_i16(glyph_data, 0);
            if num_contours < 0 {
                rewrite_composite_gids(&mut new_glyph, gid_remap);
            }
        }

        new_glyf.extend_from_slice(&new_glyph);

        // Pad to 4-byte boundary (required for loca to work correctly)
        while new_glyf.len() % 4 != 0 {
            new_glyf.push(0);
        }
    }

    // Final offset (marks end of last glyph)
    new_offsets.push(new_glyf.len() as u32);

    (new_glyf, new_offsets)
}

fn rewrite_composite_gids(glyph_data: &mut [u8], gid_remap: &HashMap<u16, u16>) {
    let mut pos = 10; // skip header

    loop {
        if pos + 4 > glyph_data.len() { break; }
        let flags = read_u16(glyph_data, pos);
        let old_gid = read_u16(glyph_data, pos + 2);

        // Rewrite the component GID
        if let Some(&new_gid) = gid_remap.get(&old_gid) {
            write_u16(glyph_data, pos + 2, new_gid);
        }

        pos += 4;

        if flags & 0x0001 != 0 { pos += 4; } else { pos += 2; }
        if flags & 0x0008 != 0 { pos += 2; }
        else if flags & 0x0040 != 0 { pos += 4; }
        else if flags & 0x0080 != 0 { pos += 8; }

        if flags & 0x0020 == 0 { break; }
    }
}

fn build_loca(offsets: &[u32], format: i16) -> Vec<u8> {
    let mut data = Vec::new();
    if format == 0 {
        for &offset in offsets {
            let short = (offset / 2) as u16;
            data.extend_from_slice(&short.to_be_bytes());
        }
    } else {
        for &offset in offsets {
            data.extend_from_slice(&offset.to_be_bytes());
        }
    }
    data
}

fn rebuild_hmtx(
    hmtx: &[u8],
    needed_gids: &BTreeSet<u16>,
    num_h_metrics: usize,
) -> Vec<u8> {
    let mut data = Vec::new();

    for &old_gid in needed_gids {
        let idx = old_gid as usize;
        if idx < num_h_metrics {
            // Full metric: advance_width (u16) + lsb (i16)
            let offset = idx * 4;
            if offset + 4 <= hmtx.len() {
                data.extend_from_slice(&hmtx[offset..offset + 4]);
            } else {
                data.extend_from_slice(&[0, 0, 0, 0]);
            }
        } else {
            // Only lsb — use last advance width + per-glyph lsb
            let last_aw_offset = (num_h_metrics - 1) * 4;
            let advance_width = if last_aw_offset + 2 <= hmtx.len() {
                &hmtx[last_aw_offset..last_aw_offset + 2]
            } else {
                &[0, 0]
            };
            let lsb_offset = num_h_metrics * 4 + (idx - num_h_metrics) * 2;
            let lsb = if lsb_offset + 2 <= hmtx.len() {
                &hmtx[lsb_offset..lsb_offset + 2]
            } else {
                &[0, 0]
            };
            data.extend_from_slice(advance_width);
            data.extend_from_slice(lsb);
        }
    }

    data
}

fn build_cmap_format4(char_to_gid: &[(u16, u16)]) -> Vec<u8> {
    // Build a cmap table with a single Format 4 subtable
    // Platform 3 (Windows), Encoding 1 (Unicode BMP)
    let mut sorted = char_to_gid.to_vec();
    sorted.sort_by_key(|(ch, _)| *ch);

    // Build segments — each segment is a contiguous run of codepoints
    let mut segments: Vec<(u16, u16, Vec<u16>)> = Vec::new(); // (start, end, gids)

    for &(ch, gid) in &sorted {
        if let Some(last) = segments.last_mut() {
            if ch == last.1 + 1 {
                last.1 = ch;
                last.2.push(gid);
                continue;
            }
        }
        segments.push((ch, ch, vec![gid]));
    }

    // Add sentinel segment (0xFFFF)
    segments.push((0xFFFF, 0xFFFF, vec![0]));

    let seg_count = segments.len() as u16;
    let seg_count_x2 = seg_count * 2;
    // Compute search parameters per TrueType spec
    let entry_selector = if seg_count > 0 { (seg_count as f64).log2().floor() as u16 } else { 0 };
    let search_range = (1u16 << entry_selector) * 2;
    let range_shift = seg_count_x2.saturating_sub(search_range);

    // Use glyphIdArray for all segments (idRangeOffset pointing to array)
    // Simpler: use idDelta for single-glyph segments, idRangeOffset for others
    // Simplest approach: use glyphIdArray for everything

    let mut glyph_id_array: Vec<u16> = Vec::new();
    let mut end_codes: Vec<u16> = Vec::new();
    let mut start_codes: Vec<u16> = Vec::new();
    let mut id_deltas: Vec<i16> = Vec::new();
    let mut id_range_offsets: Vec<u16> = Vec::new();

    for (i, (start, end, gids)) in segments.iter().enumerate() {
        start_codes.push(*start);
        end_codes.push(*end);

        if *start == 0xFFFF {
            // Sentinel
            id_deltas.push(1);
            id_range_offsets.push(0);
        } else if gids.len() == 1 {
            // Single char — use idDelta
            let delta = gids[0] as i32 - *start as i32;
            id_deltas.push(delta as i16);
            id_range_offsets.push(0);
        } else {
            // Range — use idRangeOffset into glyphIdArray
            id_deltas.push(0);
            // Offset from current position in idRangeOffset array to glyphIdArray
            let remaining_offsets = (segments.len() - i) as u16;
            let offset = (remaining_offsets + glyph_id_array.len() as u16) * 2;
            id_range_offsets.push(offset);
            glyph_id_array.extend_from_slice(gids);
        }
    }

    // Build the subtable
    let subtable_len = 14 + seg_count as usize * 8 + glyph_id_array.len() * 2;
    let mut subtable: Vec<u8> = Vec::new();
    subtable.extend_from_slice(&4u16.to_be_bytes()); // format
    subtable.extend_from_slice(&(subtable_len as u16).to_be_bytes()); // length
    subtable.extend_from_slice(&0u16.to_be_bytes()); // language
    subtable.extend_from_slice(&seg_count_x2.to_be_bytes());
    subtable.extend_from_slice(&search_range.to_be_bytes());
    subtable.extend_from_slice(&entry_selector.to_be_bytes());
    subtable.extend_from_slice(&range_shift.to_be_bytes());

    for &ec in &end_codes { subtable.extend_from_slice(&ec.to_be_bytes()); }
    subtable.extend_from_slice(&0u16.to_be_bytes()); // reservedPad

    for &sc in &start_codes { subtable.extend_from_slice(&sc.to_be_bytes()); }
    for &d in &id_deltas { subtable.extend_from_slice(&d.to_be_bytes()); }
    for &r in &id_range_offsets { subtable.extend_from_slice(&r.to_be_bytes()); }
    for &g in &glyph_id_array { subtable.extend_from_slice(&g.to_be_bytes()); }

    // Build cmap header
    let mut cmap: Vec<u8> = Vec::new();
    cmap.extend_from_slice(&0u16.to_be_bytes()); // version
    cmap.extend_from_slice(&1u16.to_be_bytes()); // numTables
    // Encoding record: platform 3 (Windows), encoding 1 (Unicode BMP)
    cmap.extend_from_slice(&3u16.to_be_bytes()); // platformID
    cmap.extend_from_slice(&1u16.to_be_bytes()); // encodingID
    cmap.extend_from_slice(&12u32.to_be_bytes()); // offset to subtable
    cmap.extend_from_slice(&subtable);

    cmap
}

fn rebuild_head(head: &[u8], new_loca_format: i16) -> Vec<u8> {
    let mut new_head = head.to_vec();
    // Zero out checkSumAdjustment (offset 8, 4 bytes) — will be fixed later
    write_u32(&mut new_head, 8, 0);
    // Update indexToLocFormat (offset 50)
    write_i16(&mut new_head, 50, new_loca_format);
    new_head
}

fn rebuild_hhea(hhea: &[u8], new_num_glyphs: u16) -> Vec<u8> {
    let mut new_hhea = hhea.to_vec();
    // Pad to 36 bytes if needed (minimum hhea size)
    while new_hhea.len() < 36 {
        new_hhea.push(0);
    }
    // Update numberOfHMetrics (offset 34) — all glyphs get full metrics
    write_u16(&mut new_hhea, 34, new_num_glyphs);
    new_hhea
}

fn build_maxp(num_glyphs: u16) -> Vec<u8> {
    let mut data = vec![0u8; 32];
    // Version 1.0
    write_u32(&mut data, 0, 0x00010000);
    // numGlyphs
    write_u16(&mut data, 4, num_glyphs);
    // Fill remaining fields with reasonable defaults
    write_u16(&mut data, 6, 256);  // maxPoints
    write_u16(&mut data, 8, 64);   // maxContours
    write_u16(&mut data, 10, 256); // maxCompositePoints
    write_u16(&mut data, 12, 64);  // maxCompositeContours
    write_u16(&mut data, 14, 1);   // maxZones
    write_u16(&mut data, 16, 0);   // maxTwilightPoints
    write_u16(&mut data, 18, 64);  // maxStorage
    write_u16(&mut data, 20, 64);  // maxFunctionDefs
    write_u16(&mut data, 22, 64);  // maxInstructionDefs
    write_u16(&mut data, 24, 64);  // maxStackElements
    write_u16(&mut data, 26, 0);   // maxSizeOfInstructions
    write_u16(&mut data, 28, 64);  // maxComponentElements
    write_u16(&mut data, 30, 2);   // maxComponentDepth
    data
}

fn build_post_format3() -> Vec<u8> {
    // Format 3.0 — no glyph names (smallest possible)
    let mut data = vec![0u8; 32];
    write_u32(&mut data, 0, 0x00030000); // version 3.0
    // italicAngle, underlinePosition, underlineThickness, isFixedPitch — all 0
    data
}

fn build_minimal_name(face: &ttf_parser::Face) -> Vec<u8> {
    // Build a minimal name table with just the font family name
    let family = face.names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::FULL_NAME)
        .and_then(|n| n.to_string())
        .unwrap_or_else(|| "SubsetFont".to_string());

    let name_bytes: Vec<u8> = family.encode_utf16().flat_map(|c| c.to_be_bytes()).collect();

    let mut data = Vec::new();
    // Name table header
    data.extend_from_slice(&0u16.to_be_bytes()); // format
    data.extend_from_slice(&1u16.to_be_bytes()); // count
    let string_offset = 6 + 12; // header (6) + 1 record (12)
    data.extend_from_slice(&(string_offset as u16).to_be_bytes()); // stringOffset

    // Name record: platformID=3, encodingID=1, languageID=0x0409, nameID=4 (fullName)
    data.extend_from_slice(&3u16.to_be_bytes()); // platformID
    data.extend_from_slice(&1u16.to_be_bytes()); // encodingID
    data.extend_from_slice(&0x0409u16.to_be_bytes()); // languageID
    data.extend_from_slice(&4u16.to_be_bytes()); // nameID (full name)
    data.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes()); // length
    data.extend_from_slice(&0u16.to_be_bytes()); // offset

    // String data
    data.extend_from_slice(&name_bytes);

    data
}

// ─── TrueType File Writer ───────────────────────────────────────

fn write_ttf_file(tables: &mut [(u32, Vec<u8>)]) -> Vec<u8> {
    let num_tables = tables.len() as u16;
    let entry_selector = if num_tables > 0 { (num_tables as f64).log2().floor() as u16 } else { 0 };
    let search_range = (1u16 << entry_selector) * 16;
    let range_shift = (num_tables * 16).saturating_sub(search_range);

    // Offset table (12 bytes)
    let mut output: Vec<u8> = Vec::new();
    output.extend_from_slice(&0x00010000u32.to_be_bytes()); // sfVersion (TrueType)
    output.extend_from_slice(&num_tables.to_be_bytes());
    output.extend_from_slice(&search_range.to_be_bytes());
    output.extend_from_slice(&entry_selector.to_be_bytes());
    output.extend_from_slice(&range_shift.to_be_bytes());

    // Calculate table offsets
    let dir_size = 12 + num_tables as usize * 16;
    let mut table_offset = dir_size;

    // Pad each table to 4-byte boundary
    for (_, data) in tables.iter_mut() {
        while data.len() % 4 != 0 {
            data.push(0);
        }
    }

    // Table directory
    for (tag, data) in tables.iter() {
        output.extend_from_slice(&tag.to_be_bytes());
        let checksum = calc_table_checksum(data);
        output.extend_from_slice(&checksum.to_be_bytes());
        output.extend_from_slice(&(table_offset as u32).to_be_bytes());
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        table_offset += data.len();
    }

    // Table data
    for (_, data) in tables.iter() {
        output.extend_from_slice(data);
    }

    // Fix head checkSumAdjustment
    fix_head_checksum(&mut output, tables);

    output
}

fn calc_table_checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 4 <= data.len() {
        sum = sum.wrapping_add(read_u32(data, i));
        i += 4;
    }
    // Handle remaining bytes
    if i < data.len() {
        let mut last = [0u8; 4];
        for (j, &b) in data[i..].iter().enumerate() {
            last[j] = b;
        }
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

fn fix_head_checksum(output: &mut [u8], tables: &[(u32, Vec<u8>)]) {
    // Find the head table offset in the directory
    let num_tables = read_u16(output, 4) as usize;
    let head_tag = tag_u32(b"head");

    for i in 0..num_tables {
        let dir_offset = 12 + i * 16;
        let tag = read_u32(output, dir_offset);
        if tag == head_tag {
            let table_offset = read_u32(output, dir_offset + 8) as usize;

            // Calculate file checksum
            let file_checksum = calc_table_checksum(output);
            let adjustment = 0xB1B0AFBAu32.wrapping_sub(file_checksum);

            // Write checkSumAdjustment at head table offset + 8
            if table_offset + 12 <= output.len() {
                write_u32(output, table_offset + 8, adjustment);
            }

            // Update the head table checksum in the directory
            // First, find the head table data to recalculate
            let head_data_len = read_u32(output, dir_offset + 12) as usize;
            if table_offset + head_data_len <= output.len() {
                let checksum = calc_table_checksum(&output[table_offset..table_offset + head_data_len]);
                write_u32(output, dir_offset + 4, checksum);
            }

            break;
        }
    }

    // Suppress unused variable warning
    let _ = tables;
}

// ─── Byte Helpers ───────────────────────────────────────────────

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

fn read_i16(data: &[u8], offset: usize) -> i16 {
    i16::from_be_bytes([data[offset], data[offset + 1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

fn write_u16(data: &mut [u8], offset: usize, val: u16) {
    let bytes = val.to_be_bytes();
    data[offset] = bytes[0];
    data[offset + 1] = bytes[1];
}

fn write_i16(data: &mut [u8], offset: usize, val: i16) {
    let bytes = val.to_be_bytes();
    data[offset] = bytes[0];
    data[offset + 1] = bytes[1];
}

fn write_u32(data: &mut [u8], offset: usize, val: u32) {
    let bytes = val.to_be_bytes();
    data[offset] = bytes[0];
    data[offset + 1] = bytes[1];
    data[offset + 2] = bytes[2];
    data[offset + 3] = bytes[3];
}

fn tag_u32(tag: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*tag)
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_u32() {
        assert_eq!(tag_u32(b"glyf"), 0x676C7966);
        assert_eq!(tag_u32(b"head"), 0x68656164);
    }

    #[test]
    fn test_calc_table_checksum() {
        // Known checksum for "ABCD" (0x41424344)
        let data = b"ABCD";
        assert_eq!(calc_table_checksum(data), 0x41424344);
    }

    #[test]
    fn test_build_post_format3() {
        let data = build_post_format3();
        assert_eq!(data.len(), 32);
        assert_eq!(read_u32(&data, 0), 0x00030000);
    }

    #[test]
    fn test_build_maxp() {
        let data = build_maxp(42);
        assert_eq!(read_u32(&data, 0), 0x00010000);
        assert_eq!(read_u16(&data, 4), 42);
    }

    #[test]
    fn test_build_loca_short() {
        let offsets = vec![0, 100, 200, 300];
        let data = build_loca(&offsets, 0);
        // Short format: each offset / 2 stored as u16
        assert_eq!(data.len(), 8); // 4 entries × 2 bytes
        assert_eq!(read_u16(&data, 0), 0);
        assert_eq!(read_u16(&data, 2), 50);
        assert_eq!(read_u16(&data, 4), 100);
        assert_eq!(read_u16(&data, 6), 150);
    }

    #[test]
    fn test_build_loca_long() {
        let offsets = vec![0, 100, 200, 300];
        let data = build_loca(&offsets, 1);
        assert_eq!(data.len(), 16); // 4 entries × 4 bytes
        assert_eq!(read_u32(&data, 0), 0);
        assert_eq!(read_u32(&data, 4), 100);
        assert_eq!(read_u32(&data, 8), 200);
        assert_eq!(read_u32(&data, 12), 300);
    }

    #[test]
    fn test_cmap_format4_single_char() {
        let entries = vec![(65u16, 1u16)]; // 'A' → gid 1
        let cmap = build_cmap_format4(&entries);

        // Should be a valid cmap table
        assert_eq!(read_u16(&cmap, 0), 0); // version
        assert_eq!(read_u16(&cmap, 2), 1); // numTables

        // Encoding record
        assert_eq!(read_u16(&cmap, 4), 3); // platformID = Windows
        assert_eq!(read_u16(&cmap, 6), 1); // encodingID = Unicode BMP

        // Subtable format should be 4
        let subtable_offset = read_u32(&cmap, 8) as usize;
        assert_eq!(read_u16(&cmap, subtable_offset), 4);
    }
}

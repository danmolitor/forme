//! # SVG Parser
//!
//! Parses a subset of SVG into drawing commands that can be rendered to PDF.
//! Supports: rect, circle, ellipse, line, polyline, polygon, path, g (group).
//! Path commands: M, L, H, V, C, Q, A, Z (absolute + relative).

use quick_xml::events::Event;
use quick_xml::Reader;

/// A parsed SVG viewBox.
#[derive(Debug, Clone, Copy)]
pub struct ViewBox {
    pub min_x: f64,
    pub min_y: f64,
    pub width: f64,
    pub height: f64,
}

/// Drawing commands produced by the SVG parser.
#[derive(Debug, Clone)]
pub enum SvgCommand {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    CurveTo(f64, f64, f64, f64, f64, f64),
    ClosePath,
    SetFill(f64, f64, f64),
    SetFillNone,
    SetStroke(f64, f64, f64),
    SetStrokeNone,
    SetStrokeWidth(f64),
    Fill,
    Stroke,
    FillAndStroke,
    SetLineCap(u32),
    SetLineJoin(u32),
    SaveState,
    RestoreState,
}

/// Parse a viewBox string like "0 0 100 100".
pub fn parse_view_box(s: &str) -> Option<ViewBox> {
    let parts: Vec<f64> = s
        .split_whitespace()
        .filter_map(|p| p.parse::<f64>().ok())
        .collect();
    if parts.len() == 4 {
        Some(ViewBox {
            min_x: parts[0],
            min_y: parts[1],
            width: parts[2],
            height: parts[3],
        })
    } else {
        None
    }
}

/// Parse SVG XML content into drawing commands.
pub fn parse_svg(
    content: &str,
    _view_box: ViewBox,
    _target_width: f64,
    _target_height: f64,
) -> Vec<SvgCommand> {
    let mut commands = Vec::new();
    let mut reader = Reader::from_str(content);

    let mut fill_stack: Vec<Option<(f64, f64, f64)>> = vec![Some((0.0, 0.0, 0.0))];
    let mut stroke_stack: Vec<Option<(f64, f64, f64)>> = vec![None];
    let mut stroke_width_stack: Vec<f64> = vec![1.0];

    let mut buf = Vec::new();

    loop {
        let event = reader.read_event_into(&mut buf);
        let (e_ref, is_start) = match &event {
            Ok(Event::Start(e)) => (Some(e), true),
            Ok(Event::Empty(e)) => (Some(e), false),
            Ok(Event::End(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag_name == "g" {
                    fill_stack.pop();
                    stroke_stack.pop();
                    stroke_width_stack.pop();
                    commands.push(SvgCommand::RestoreState);
                }
                buf.clear();
                continue;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {
                buf.clear();
                continue;
            }
        };
        if let Some(e) = e_ref {
            let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

            // Parse style attributes
            let fill = get_attr(e, "fill");
            let stroke = get_attr(e, "stroke");
            let sw = get_attr(e, "stroke-width");

            let current_fill = if let Some(ref f) = fill {
                if f == "none" {
                    None
                } else {
                    parse_svg_color(f).or(*fill_stack.last().unwrap_or(&Some((0.0, 0.0, 0.0))))
                }
            } else {
                *fill_stack.last().unwrap_or(&Some((0.0, 0.0, 0.0)))
            };

            let current_stroke = if let Some(ref s) = stroke {
                if s == "none" {
                    None
                } else {
                    parse_svg_color(s).or(*stroke_stack.last().unwrap_or(&None))
                }
            } else {
                *stroke_stack.last().unwrap_or(&None)
            };

            let current_sw = sw
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(*stroke_width_stack.last().unwrap_or(&1.0));

            match tag_name.as_str() {
                "g" if is_start => {
                    commands.push(SvgCommand::SaveState);
                    fill_stack.push(current_fill);
                    stroke_stack.push(current_stroke);
                    stroke_width_stack.push(current_sw);
                }
                "rect" => {
                    let x = get_attr_f64(e, "x").unwrap_or(0.0);
                    let y = get_attr_f64(e, "y").unwrap_or(0.0);
                    let w = get_attr_f64(e, "width").unwrap_or(0.0);
                    let h = get_attr_f64(e, "height").unwrap_or(0.0);

                    emit_shape(
                        &mut commands,
                        current_fill,
                        current_stroke,
                        current_sw,
                        || {
                            vec![
                                SvgCommand::MoveTo(x, y),
                                SvgCommand::LineTo(x + w, y),
                                SvgCommand::LineTo(x + w, y + h),
                                SvgCommand::LineTo(x, y + h),
                                SvgCommand::ClosePath,
                            ]
                        },
                    );
                }
                "circle" => {
                    let cx = get_attr_f64(e, "cx").unwrap_or(0.0);
                    let cy = get_attr_f64(e, "cy").unwrap_or(0.0);
                    let r = get_attr_f64(e, "r").unwrap_or(0.0);

                    emit_shape(
                        &mut commands,
                        current_fill,
                        current_stroke,
                        current_sw,
                        || ellipse_commands(cx, cy, r, r),
                    );
                }
                "ellipse" => {
                    let cx = get_attr_f64(e, "cx").unwrap_or(0.0);
                    let cy = get_attr_f64(e, "cy").unwrap_or(0.0);
                    let rx = get_attr_f64(e, "rx").unwrap_or(0.0);
                    let ry = get_attr_f64(e, "ry").unwrap_or(0.0);

                    emit_shape(
                        &mut commands,
                        current_fill,
                        current_stroke,
                        current_sw,
                        || ellipse_commands(cx, cy, rx, ry),
                    );
                }
                "line" => {
                    let x1 = get_attr_f64(e, "x1").unwrap_or(0.0);
                    let y1 = get_attr_f64(e, "y1").unwrap_or(0.0);
                    let x2 = get_attr_f64(e, "x2").unwrap_or(0.0);
                    let y2 = get_attr_f64(e, "y2").unwrap_or(0.0);

                    // Lines only have stroke, no fill
                    emit_shape(&mut commands, None, current_stroke, current_sw, || {
                        vec![SvgCommand::MoveTo(x1, y1), SvgCommand::LineTo(x2, y2)]
                    });
                }
                "polyline" | "polygon" => {
                    let points_str = get_attr(e, "points").unwrap_or_default();
                    let points = parse_points(&points_str);
                    if !points.is_empty() {
                        let close = tag_name == "polygon";
                        emit_shape(
                            &mut commands,
                            current_fill,
                            current_stroke,
                            current_sw,
                            || {
                                let mut cmds = Vec::new();
                                cmds.push(SvgCommand::MoveTo(points[0].0, points[0].1));
                                for &(px, py) in &points[1..] {
                                    cmds.push(SvgCommand::LineTo(px, py));
                                }
                                if close {
                                    cmds.push(SvgCommand::ClosePath);
                                }
                                cmds
                            },
                        );
                    }
                }
                "path" => {
                    let d = get_attr(e, "d").unwrap_or_default();
                    let path_cmds = parse_path_d(&d);
                    if !path_cmds.is_empty() {
                        emit_shape(
                            &mut commands,
                            current_fill,
                            current_stroke,
                            current_sw,
                            || path_cmds.clone(),
                        );
                    }
                }
                _ => {}
            }
        }
        buf.clear();
    }

    commands
}

fn emit_shape(
    commands: &mut Vec<SvgCommand>,
    fill: Option<(f64, f64, f64)>,
    stroke: Option<(f64, f64, f64)>,
    stroke_width: f64,
    path_fn: impl FnOnce() -> Vec<SvgCommand>,
) {
    let has_fill = fill.is_some();
    let has_stroke = stroke.is_some();

    if !has_fill && !has_stroke {
        return;
    }

    commands.push(SvgCommand::SaveState);

    if let Some((r, g, b)) = fill {
        commands.push(SvgCommand::SetFill(r, g, b));
    }
    if let Some((r, g, b)) = stroke {
        commands.push(SvgCommand::SetStroke(r, g, b));
        commands.push(SvgCommand::SetStrokeWidth(stroke_width));
    }

    commands.extend(path_fn());

    match (has_fill, has_stroke) {
        (true, true) => commands.push(SvgCommand::FillAndStroke),
        (true, false) => commands.push(SvgCommand::Fill),
        (false, true) => commands.push(SvgCommand::Stroke),
        _ => {}
    }

    commands.push(SvgCommand::RestoreState);
}

/// Generate cubic bezier commands to approximate an ellipse.
pub fn ellipse_commands(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<SvgCommand> {
    let k: f64 = 0.5522847498;
    let kx = rx * k;
    let ky = ry * k;

    vec![
        SvgCommand::MoveTo(cx + rx, cy),
        SvgCommand::CurveTo(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
        SvgCommand::CurveTo(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
        SvgCommand::CurveTo(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
        SvgCommand::CurveTo(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        SvgCommand::ClosePath,
    ]
}

/// Convert an SVG arc command to cubic bezier curves.
/// Implements W3C SVG spec F.6.5/F.6.6 (endpoint-to-center parameterization).
#[allow(clippy::too_many_arguments)]
fn svg_arc_to_curves(
    x1: f64,
    y1: f64,
    mut rx: f64,
    mut ry: f64,
    x_rotation_deg: f64,
    large_arc: bool,
    sweep: bool,
    x2: f64,
    y2: f64,
) -> Vec<SvgCommand> {
    // F.6.2: If endpoints are identical, skip
    if (x1 - x2).abs() < 1e-10 && (y1 - y2).abs() < 1e-10 {
        return vec![];
    }
    // F.6.2: If either radius is zero, treat as line
    if rx.abs() < 1e-10 || ry.abs() < 1e-10 {
        return vec![SvgCommand::LineTo(x2, y2)];
    }

    rx = rx.abs();
    ry = ry.abs();

    let phi = x_rotation_deg.to_radians();
    let cos_phi = phi.cos();
    let sin_phi = phi.sin();

    // F.6.5.1: Compute (x1', y1')
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;

    // F.6.6.2: Ensure radii are large enough
    let x1p2 = x1p * x1p;
    let y1p2 = y1p * y1p;
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let lambda = x1p2 / rx2 + y1p2 / ry2;
    if lambda > 1.0 {
        let lambda_sqrt = lambda.sqrt();
        rx *= lambda_sqrt;
        ry *= lambda_sqrt;
    }

    let rx2 = rx * rx;
    let ry2 = ry * ry;

    // F.6.5.2: Compute center point (cx', cy')
    let num = (rx2 * ry2 - rx2 * y1p2 - ry2 * x1p2).max(0.0);
    let den = rx2 * y1p2 + ry2 * x1p2;
    let sq = if den.abs() < 1e-10 {
        0.0
    } else {
        (num / den).sqrt()
    };
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let cxp = sign * sq * (rx * y1p / ry);
    let cyp = sign * sq * -(ry * x1p / rx);

    // F.6.5.3: Compute center point (cx, cy)
    let cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) / 2.0;

    // F.6.5.5/F.6.5.6: Compute theta1 and dtheta
    let theta1 = angle_between(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = angle_between(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );

    if !sweep && dtheta > 0.0 {
        dtheta -= std::f64::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f64::consts::TAU;
    }

    // Split arc into segments of at most PI/2
    let n_segs = (dtheta.abs() / (std::f64::consts::FRAC_PI_2)).ceil() as usize;
    let n_segs = n_segs.max(1);
    let d_per_seg = dtheta / n_segs as f64;

    let mut commands = Vec::new();
    let mut theta = theta1;

    for _ in 0..n_segs {
        let t1 = theta;
        let t2 = theta + d_per_seg;

        // Cubic bezier approximation of arc segment
        let alpha = (d_per_seg / 4.0).tan() * 4.0 / 3.0;

        let cos_t1 = t1.cos();
        let sin_t1 = t1.sin();
        let cos_t2 = t2.cos();
        let sin_t2 = t2.sin();

        // Points on the unit circle
        let ep1x = cos_t1 - alpha * sin_t1;
        let ep1y = sin_t1 + alpha * cos_t1;
        let ep2x = cos_t2 + alpha * sin_t2;
        let ep2y = sin_t2 - alpha * cos_t2;

        // Scale by radii, rotate, translate
        let cp1x = cos_phi * rx * ep1x - sin_phi * ry * ep1y + cx;
        let cp1y = sin_phi * rx * ep1x + cos_phi * ry * ep1y + cy;
        let cp2x = cos_phi * rx * ep2x - sin_phi * ry * ep2y + cx;
        let cp2y = sin_phi * rx * ep2x + cos_phi * ry * ep2y + cy;
        let ex = cos_phi * rx * cos_t2 - sin_phi * ry * sin_t2 + cx;
        let ey = sin_phi * rx * cos_t2 + cos_phi * ry * sin_t2 + cy;

        commands.push(SvgCommand::CurveTo(cp1x, cp1y, cp2x, cp2y, ex, ey));

        theta = t2;
    }

    commands
}

/// Compute the angle between two vectors.
fn angle_between(ux: f64, uy: f64, vx: f64, vy: f64) -> f64 {
    let dot = ux * vx + uy * vy;
    let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
    if len.abs() < 1e-10 {
        return 0.0;
    }
    let cos_val = (dot / len).clamp(-1.0, 1.0);
    let angle = cos_val.acos();
    if ux * vy - uy * vx < 0.0 {
        -angle
    } else {
        angle
    }
}

/// Parse an SVG path `d` attribute into drawing commands.
fn parse_path_d(d: &str) -> Vec<SvgCommand> {
    let mut commands = Vec::new();
    let mut cur_x = 0.0f64;
    let mut cur_y = 0.0f64;
    let mut start_x = 0.0f64;
    let mut start_y = 0.0f64;

    let tokens = tokenize_path(d);
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i].as_str() {
            "M" => {
                if i + 2 < tokens.len() {
                    cur_x = tokens[i + 1].parse().unwrap_or(0.0);
                    cur_y = tokens[i + 2].parse().unwrap_or(0.0);
                    start_x = cur_x;
                    start_y = cur_y;
                    commands.push(SvgCommand::MoveTo(cur_x, cur_y));
                    i += 3;
                    // Implicit LineTo for subsequent coordinate pairs
                    while i + 1 < tokens.len() && is_number(&tokens[i]) {
                        cur_x = tokens[i].parse().unwrap_or(0.0);
                        cur_y = tokens[i + 1].parse().unwrap_or(0.0);
                        commands.push(SvgCommand::LineTo(cur_x, cur_y));
                        i += 2;
                    }
                } else {
                    i += 1;
                }
            }
            "m" => {
                if i + 2 < tokens.len() {
                    cur_x += tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    cur_y += tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                    start_x = cur_x;
                    start_y = cur_y;
                    commands.push(SvgCommand::MoveTo(cur_x, cur_y));
                    i += 3;
                    while i + 1 < tokens.len() && is_number(&tokens[i]) {
                        cur_x += tokens[i].parse::<f64>().unwrap_or(0.0);
                        cur_y += tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                        commands.push(SvgCommand::LineTo(cur_x, cur_y));
                        i += 2;
                    }
                } else {
                    i += 1;
                }
            }
            "L" => {
                i += 1;
                while i + 1 < tokens.len() && is_number(&tokens[i]) {
                    cur_x = tokens[i].parse().unwrap_or(0.0);
                    cur_y = tokens[i + 1].parse().unwrap_or(0.0);
                    commands.push(SvgCommand::LineTo(cur_x, cur_y));
                    i += 2;
                }
            }
            "l" => {
                i += 1;
                while i + 1 < tokens.len() && is_number(&tokens[i]) {
                    cur_x += tokens[i].parse::<f64>().unwrap_or(0.0);
                    cur_y += tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    commands.push(SvgCommand::LineTo(cur_x, cur_y));
                    i += 2;
                }
            }
            "H" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    cur_x = tokens[i].parse().unwrap_or(0.0);
                    commands.push(SvgCommand::LineTo(cur_x, cur_y));
                    i += 1;
                }
            }
            "h" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    cur_x += tokens[i].parse::<f64>().unwrap_or(0.0);
                    commands.push(SvgCommand::LineTo(cur_x, cur_y));
                    i += 1;
                }
            }
            "V" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    cur_y = tokens[i].parse().unwrap_or(0.0);
                    commands.push(SvgCommand::LineTo(cur_x, cur_y));
                    i += 1;
                }
            }
            "v" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    cur_y += tokens[i].parse::<f64>().unwrap_or(0.0);
                    commands.push(SvgCommand::LineTo(cur_x, cur_y));
                    i += 1;
                }
            }
            "C" => {
                i += 1;
                while i + 5 < tokens.len() && is_number(&tokens[i]) {
                    let x1 = tokens[i].parse().unwrap_or(0.0);
                    let y1 = tokens[i + 1].parse().unwrap_or(0.0);
                    let x2 = tokens[i + 2].parse().unwrap_or(0.0);
                    let y2 = tokens[i + 3].parse().unwrap_or(0.0);
                    cur_x = tokens[i + 4].parse().unwrap_or(0.0);
                    cur_y = tokens[i + 5].parse().unwrap_or(0.0);
                    commands.push(SvgCommand::CurveTo(x1, y1, x2, y2, cur_x, cur_y));
                    i += 6;
                }
            }
            "c" => {
                i += 1;
                while i + 5 < tokens.len() && is_number(&tokens[i]) {
                    let x1 = cur_x + tokens[i].parse::<f64>().unwrap_or(0.0);
                    let y1 = cur_y + tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    let x2 = cur_x + tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                    let y2 = cur_y + tokens[i + 3].parse::<f64>().unwrap_or(0.0);
                    cur_x += tokens[i + 4].parse::<f64>().unwrap_or(0.0);
                    cur_y += tokens[i + 5].parse::<f64>().unwrap_or(0.0);
                    commands.push(SvgCommand::CurveTo(x1, y1, x2, y2, cur_x, cur_y));
                    i += 6;
                }
            }
            "Q" => {
                i += 1;
                while i + 3 < tokens.len() && is_number(&tokens[i]) {
                    let qx = tokens[i].parse::<f64>().unwrap_or(0.0);
                    let qy = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    let end_x = tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                    let end_y = tokens[i + 3].parse::<f64>().unwrap_or(0.0);
                    // Convert quadratic to cubic
                    let c1x = cur_x + (2.0 / 3.0) * (qx - cur_x);
                    let c1y = cur_y + (2.0 / 3.0) * (qy - cur_y);
                    let c2x = end_x + (2.0 / 3.0) * (qx - end_x);
                    let c2y = end_y + (2.0 / 3.0) * (qy - end_y);
                    cur_x = end_x;
                    cur_y = end_y;
                    commands.push(SvgCommand::CurveTo(c1x, c1y, c2x, c2y, cur_x, cur_y));
                    i += 4;
                }
            }
            "q" => {
                i += 1;
                while i + 3 < tokens.len() && is_number(&tokens[i]) {
                    let qx = cur_x + tokens[i].parse::<f64>().unwrap_or(0.0);
                    let qy = cur_y + tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    let end_x = cur_x + tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                    let end_y = cur_y + tokens[i + 3].parse::<f64>().unwrap_or(0.0);
                    let c1x = cur_x + (2.0 / 3.0) * (qx - cur_x);
                    let c1y = cur_y + (2.0 / 3.0) * (qy - cur_y);
                    let c2x = end_x + (2.0 / 3.0) * (qx - end_x);
                    let c2y = end_y + (2.0 / 3.0) * (qy - end_y);
                    cur_x = end_x;
                    cur_y = end_y;
                    commands.push(SvgCommand::CurveTo(c1x, c1y, c2x, c2y, cur_x, cur_y));
                    i += 4;
                }
            }
            "A" => {
                i += 1;
                while i + 6 < tokens.len() && is_number(&tokens[i]) {
                    let rx = tokens[i].parse::<f64>().unwrap_or(0.0);
                    let ry = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    let x_rotation = tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                    let large_arc = tokens[i + 3].parse::<f64>().unwrap_or(0.0) != 0.0;
                    let sweep = tokens[i + 4].parse::<f64>().unwrap_or(0.0) != 0.0;
                    let end_x = tokens[i + 5].parse::<f64>().unwrap_or(0.0);
                    let end_y = tokens[i + 6].parse::<f64>().unwrap_or(0.0);
                    commands.extend(svg_arc_to_curves(
                        cur_x, cur_y, rx, ry, x_rotation, large_arc, sweep, end_x, end_y,
                    ));
                    cur_x = end_x;
                    cur_y = end_y;
                    i += 7;
                }
            }
            "a" => {
                i += 1;
                while i + 6 < tokens.len() && is_number(&tokens[i]) {
                    let rx = tokens[i].parse::<f64>().unwrap_or(0.0);
                    let ry = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    let x_rotation = tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                    let large_arc = tokens[i + 3].parse::<f64>().unwrap_or(0.0) != 0.0;
                    let sweep = tokens[i + 4].parse::<f64>().unwrap_or(0.0) != 0.0;
                    let end_x = cur_x + tokens[i + 5].parse::<f64>().unwrap_or(0.0);
                    let end_y = cur_y + tokens[i + 6].parse::<f64>().unwrap_or(0.0);
                    commands.extend(svg_arc_to_curves(
                        cur_x, cur_y, rx, ry, x_rotation, large_arc, sweep, end_x, end_y,
                    ));
                    cur_x = end_x;
                    cur_y = end_y;
                    i += 7;
                }
            }
            "Z" | "z" => {
                commands.push(SvgCommand::ClosePath);
                cur_x = start_x;
                cur_y = start_y;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    commands
}

/// Tokenize a path `d` string into commands and numbers.
fn tokenize_path(d: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = d.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch.is_alphabetic() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            tokens.push(ch.to_string());
            i += 1;
        } else if ch == '-'
            && !current.is_empty()
            && !current.ends_with('e')
            && !current.ends_with('E')
        {
            // Negative sign starts a new number (unless after exponent)
            tokens.push(current.clone());
            current.clear();
            current.push(ch);
            i += 1;
        } else if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' {
            current.push(ch);
            i += 1;
        } else if ch == ',' || ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            i += 1;
        } else {
            i += 1;
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn is_number(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

/// Parse an SVG color string (hex, named colors).
fn parse_svg_color(s: &str) -> Option<(f64, f64, f64)> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()? as f64 / 255.0;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()? as f64 / 255.0;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()? as f64 / 255.0;
                Some((r, g, b))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f64 / 255.0;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f64 / 255.0;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f64 / 255.0;
                Some((r, g, b))
            }
            _ => None,
        }
    } else if s.starts_with("rgb(") {
        let inner = s.trim_start_matches("rgb(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse::<f64>().ok()? / 255.0;
            let g = parts[1].trim().parse::<f64>().ok()? / 255.0;
            let b = parts[2].trim().parse::<f64>().ok()? / 255.0;
            Some((r, g, b))
        } else {
            None
        }
    } else {
        // Named colors
        match s.to_lowercase().as_str() {
            "black" => Some((0.0, 0.0, 0.0)),
            "white" => Some((1.0, 1.0, 1.0)),
            "red" => Some((1.0, 0.0, 0.0)),
            "green" => Some((0.0, 0.502, 0.0)),
            "blue" => Some((0.0, 0.0, 1.0)),
            "yellow" => Some((1.0, 1.0, 0.0)),
            "gray" | "grey" => Some((0.502, 0.502, 0.502)),
            "orange" => Some((1.0, 0.647, 0.0)),
            "purple" => Some((0.502, 0.0, 0.502)),
            "cyan" => Some((0.0, 1.0, 1.0)),
            "magenta" => Some((1.0, 0.0, 1.0)),
            _ => None,
        }
    }
}

/// Parse SVG points attribute (e.g., "10,20 30,40").
fn parse_points(s: &str) -> Vec<(f64, f64)> {
    let nums: Vec<f64> = s
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    nums.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0], c[1]))
        .collect()
}

/// Helper to get an attribute value from a quick-xml BytesStart.
fn get_attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            return String::from_utf8(attr.value.to_vec()).ok();
        }
    }
    None
}

fn get_attr_f64(e: &quick_xml::events::BytesStart, name: &str) -> Option<f64> {
    get_attr(e, name).and_then(|s| s.parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_view_box() {
        let vb = parse_view_box("0 0 100 200").unwrap();
        assert!((vb.min_x - 0.0).abs() < 0.001);
        assert!((vb.width - 100.0).abs() < 0.001);
        assert!((vb.height - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_view_box_invalid() {
        assert!(parse_view_box("bad").is_none());
    }

    #[test]
    fn test_parse_rect() {
        let cmds = parse_svg(
            r##"<rect x="10" y="20" width="100" height="50" fill="#ff0000"/>"##,
            ViewBox {
                min_x: 0.0,
                min_y: 0.0,
                width: 200.0,
                height: 200.0,
            },
            200.0,
            200.0,
        );
        assert!(!cmds.is_empty());
        // Should have SaveState, SetFill, MoveTo, LineTo..., Fill, RestoreState
        assert!(cmds
            .iter()
            .any(|c| matches!(c, SvgCommand::SetFill(r, _, _) if (*r - 1.0).abs() < 0.01)));
    }

    #[test]
    fn test_parse_circle() {
        let cmds = parse_svg(
            r#"<circle cx="50" cy="50" r="25" fill="blue"/>"#,
            ViewBox {
                min_x: 0.0,
                min_y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            100.0,
            100.0,
        );
        assert!(!cmds.is_empty());
        assert!(cmds.iter().any(|c| matches!(c, SvgCommand::CurveTo(..))));
    }

    #[test]
    fn test_parse_path_m_l_z() {
        let cmds = parse_path_d("M 10 20 L 30 40 Z");
        assert!(
            matches!(cmds[0], SvgCommand::MoveTo(x, y) if (x - 10.0).abs() < 0.001 && (y - 20.0).abs() < 0.001)
        );
        assert!(
            matches!(cmds[1], SvgCommand::LineTo(x, y) if (x - 30.0).abs() < 0.001 && (y - 40.0).abs() < 0.001)
        );
        assert!(matches!(cmds[2], SvgCommand::ClosePath));
    }

    #[test]
    fn test_parse_path_relative() {
        let cmds = parse_path_d("m 10 20 l 5 5 z");
        assert!(
            matches!(cmds[0], SvgCommand::MoveTo(x, y) if (x - 10.0).abs() < 0.001 && (y - 20.0).abs() < 0.001)
        );
        assert!(
            matches!(cmds[1], SvgCommand::LineTo(x, y) if (x - 15.0).abs() < 0.001 && (y - 25.0).abs() < 0.001)
        );
    }

    #[test]
    fn test_parse_hex_color() {
        let (r, g, b) = parse_svg_color("#ff0000").unwrap();
        assert!((r - 1.0).abs() < 0.01);
        assert!((g - 0.0).abs() < 0.01);
        assert!((b - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_line() {
        let cmds = parse_svg(
            r#"<line x1="0" y1="0" x2="100" y2="100" stroke="black"/>"#,
            ViewBox {
                min_x: 0.0,
                min_y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            100.0,
            100.0,
        );
        assert!(!cmds.is_empty());
        assert!(cmds.iter().any(|c| matches!(c, SvgCommand::Stroke)));
    }

    #[test]
    fn test_empty_svg() {
        let cmds = parse_svg(
            "",
            ViewBox {
                min_x: 0.0,
                min_y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            100.0,
            100.0,
        );
        assert!(cmds.is_empty());
    }
}

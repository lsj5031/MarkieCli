use std::collections::HashMap;

use crate::fonts::TextMeasure;

use super::layout::{BBox, LayoutEngine, LayoutPos, flowchart_label_lines};
use super::render::{DiagramStyle, escape_xml};
use super::types::{ArrowType, EdgeStyle, FlowDirection, Flowchart, NodeShape};
use crate::layout::Rect;

/// Render a flowchart to SVG
pub fn render_flowchart(
    flowchart: &Flowchart,
    style: &DiagramStyle,
    measure: &mut impl TextMeasure,
) -> Result<(String, f32, f32), String> {
    if flowchart.nodes.is_empty() {
        return Ok(("<g></g>".to_string(), 100.0, 50.0));
    }

    let mut layout = LayoutEngine::new(measure, style.font_size);
    let (positions, edge_waypoints, bbox) = layout.layout_flowchart(flowchart);

    let mut svg = String::new();
    let padding = 20.0;
    let mut extents = vec![Rect::new(bbox.x, bbox.y, bbox.width, bbox.height)];
    let mut occupied_labels = Vec::new();
    let mut labels = String::new();
    let node_bounds: Vec<Rect> = positions
        .values()
        .map(|pos| Rect::new(pos.x, pos.y, pos.width, pos.height).with_padding(8.0))
        .collect();
    let group_bounds: Vec<Rect> = flowchart
        .subgraphs
        .iter()
        .filter_map(|group| {
            subgraph_bbox(group, &positions).map(|(x, y, w, h)| Rect::new(x, y, w, h))
        })
        .collect();
    extents.extend(group_bounds.iter().copied());
    let mut pair_totals = HashMap::new();
    let pair_key = |a: &str, b: &str| {
        if a <= b {
            (a.to_owned(), b.to_owned())
        } else {
            (b.to_owned(), a.to_owned())
        }
    };
    for edge in &flowchart.edges {
        *pair_totals
            .entry(pair_key(&edge.from, &edge.to))
            .or_insert(0_usize) += 1;
    }
    let mut pair_seen = HashMap::new();
    let mut loop_label_widths: HashMap<&str, f32> = HashMap::new();
    for edge in &flowchart.edges {
        if edge.from == edge.to {
            let width = edge.label.as_ref().map_or(0.0, |label| {
                measure
                    .measure_text(label, style.font_size * 0.85, false, false, false, None)
                    .0
            });
            let widest = loop_label_widths.entry(edge.from.as_str()).or_default();
            *widest = widest.max(width);
        }
    }
    let mut outer_lane = 0;

    let node_map: HashMap<&str, &super::types::FlowchartNode> =
        flowchart.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Draw subgraph boxes first (background layer)
    for subgraph in &flowchart.subgraphs {
        svg.push_str(&render_subgraph_box(subgraph, &positions, style));
    }

    // Draw edges (behind nodes but on top of subgraph boxes)
    for edge in &flowchart.edges {
        let from_pos = positions.get(&edge.from);
        let to_pos = positions.get(&edge.to);
        let from_node = node_map.get(edge.from.as_str());
        let to_node = node_map.get(edge.to.as_str());

        if let (Some(from), Some(to), Some(fn_), Some(tn)) = (from_pos, to_pos, from_node, to_node)
        {
            let waypoints = edge_waypoints
                .get(&(edge.from.clone(), edge.to.clone()))
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let key = pair_key(&edge.from, &edge.to);
            let route_index = *pair_seen.entry(key.clone()).or_insert(0);
            pair_seen.insert(key.clone(), route_index + 1);
            let backwards = match flowchart.direction {
                FlowDirection::TopDown => from.center().1 > to.center().1 + 1.0,
                FlowDirection::BottomUp => from.center().1 < to.center().1 - 1.0,
                FlowDirection::LeftRight => from.center().0 > to.center().0 + 1.0,
                FlowDirection::RightLeft => from.center().0 < to.center().0 - 1.0,
            };
            let use_outer_lane = edge.from != edge.to && (route_index > 0 || backwards);
            svg.push_str(&render_edge(&mut RenderEdgeContext {
                edge,
                from_node: fn_,
                from,
                to_node: tn,
                to,
                style,
                direction: &flowchart.direction,
                waypoints,
                measure,
                route_index,
                route_total: pair_totals[&key],
                outer_lane: use_outer_lane.then_some(outer_lane),
                loop_label_width: loop_label_widths
                    .get(edge.from.as_str())
                    .copied()
                    .unwrap_or(0.0),
                extents: &mut extents,
                group_bounds: &group_bounds,
                occupied_labels: &mut occupied_labels,
                node_bounds: &node_bounds,
                labels: &mut labels,
            }));
            if use_outer_lane {
                outer_lane += 1;
            }
        }
    }
    // Label backgrounds must also cover routes belonging to later edges.
    svg.push_str(&labels);

    // Draw nodes on top
    for node in &flowchart.nodes {
        if let Some(pos) = positions.get(&node.id) {
            let lines = flowchart_label_lines(&node.label, style.font_size, measure);
            svg.push_str(&render_node(&lines, &node.shape, pos, style));
        }
    }

    // Draw subgraph titles last (on top of everything) with collision avoidance
    let mut used_title_rects: Vec<Rect> = Vec::new();
    for subgraph in &flowchart.subgraphs {
        svg.push_str(&render_subgraph_title(
            subgraph,
            &positions,
            style,
            &mut used_title_rects,
        ));
    }

    extents.extend(used_title_rects);
    let min_x = extents.iter().map(|r| r.x).fold(0.0, f32::min);
    let min_y = extents.iter().map(|r| r.y).fold(0.0, f32::min);
    let shift_x = if min_x < 0.0 { padding - min_x } else { 0.0 };
    let shift_y = if min_y < 0.0 { padding - min_y } else { 0.0 };
    let total_width = extents.iter().map(|r| r.x + r.w).fold(0.0, f32::max) + shift_x + padding;
    let total_height = extents.iter().map(|r| r.y + r.h).fold(0.0, f32::max) + shift_y + padding;
    if shift_x > 0.0 || shift_y > 0.0 {
        svg = format!(r#"<g transform="translate({shift_x:.2},{shift_y:.2})">{svg}</g>"#);
    }

    Ok((svg, total_width, total_height))
}

fn render_node(
    lines: &[String],
    shape: &NodeShape,
    pos: &LayoutPos,
    style: &DiagramStyle,
) -> String {
    let mut svg = String::new();

    match shape {
        NodeShape::Rect => {
            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x, pos.y, pos.width, pos.height,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::RoundedRect => {
            let rx = 6.0_f32.min(pos.height / 4.0);
            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x, pos.y, pos.width, pos.height, rx,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Stadium => {
            let rx = pos.height / 2.0;
            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x, pos.y, pos.width, pos.height, rx,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Subroutine => {
            // Rect with vertical lines at ends
            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x, pos.y, pos.width, pos.height,
                style.node_fill, style.node_stroke
            ));
            let line_offset = 6.0;
            svg.push_str(&format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
                pos.x + line_offset, pos.y, pos.x + line_offset, pos.y + pos.height, style.node_stroke
            ));
            svg.push_str(&format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
                pos.x + pos.width - line_offset, pos.y, pos.x + pos.width - line_offset, pos.y + pos.height, style.node_stroke
            ));
        }
        NodeShape::Cylinder => {
            let cap_height = 12.0;
            let rx = pos.width / 2.0;
            let bottom_y = pos.y + pos.height - cap_height;
            // Body: left side, bottom arc, right side (filled, no top/bottom strokes)
            svg.push_str(&format!(
                r#"<path d="M {:.2} {:.2} L {:.2} {:.2} A {:.2} {:.2} 0 0 0 {:.2} {:.2} L {:.2} {:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x, pos.y + cap_height,
                pos.x, bottom_y,
                rx, cap_height,
                pos.x + pos.width, bottom_y,
                pos.x + pos.width, pos.y + cap_height,
                style.node_fill, style.node_stroke
            ));
            // Top ellipse (full, drawn on top of body)
            svg.push_str(&format!(
                r#"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x + pos.width / 2.0, pos.y + cap_height,
                rx, cap_height,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Circle => {
            let radius = pos.width.min(pos.height) / 2.0;
            let cx = pos.x + pos.width / 2.0;
            let cy = pos.y + pos.height / 2.0;
            svg.push_str(&format!(
                r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                cx, cy, radius, style.node_fill, style.node_stroke
            ));
        }
        NodeShape::DoubleCircle => {
            let radius = pos.width.min(pos.height) / 2.0 - 4.0;
            let cx = pos.x + pos.width / 2.0;
            let cy = pos.y + pos.height / 2.0;
            svg.push_str(&format!(
                r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                cx, cy, radius + 4.0, style.node_fill, style.node_stroke
            ));
            svg.push_str(&format!(
                r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                cx, cy, radius, style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Rhombus => {
            let cx = pos.x + pos.width / 2.0;
            let cy = pos.y + pos.height / 2.0;
            svg.push_str(&format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                cx, pos.y,
                pos.x + pos.width, cy,
                cx, pos.y + pos.height,
                pos.x, cy,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Hexagon => {
            let offset = 15.0_f32.min(pos.width / 4.0);
            svg.push_str(&format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x + offset, pos.y,
                pos.x + pos.width - offset, pos.y,
                pos.x + pos.width, pos.y + pos.height / 2.0,
                pos.x + pos.width - offset, pos.y + pos.height,
                pos.x + offset, pos.y + pos.height,
                pos.x, pos.y + pos.height / 2.0,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Parallelogram => {
            let offset = 20.0_f32.min(pos.width / 3.0);
            svg.push_str(&format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x + offset, pos.y,
                pos.x + pos.width, pos.y,
                pos.x + pos.width - offset, pos.y + pos.height,
                pos.x, pos.y + pos.height,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::ParallelogramAlt => {
            let offset = 20.0_f32.min(pos.width / 3.0);
            svg.push_str(&format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x, pos.y,
                pos.x + pos.width - offset, pos.y,
                pos.x + pos.width, pos.y + pos.height,
                pos.x + offset, pos.y + pos.height,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Trapezoid => {
            let offset = 15.0_f32.min(pos.width / 4.0);
            svg.push_str(&format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x + offset, pos.y,
                pos.x + pos.width - offset, pos.y,
                pos.x + pos.width, pos.y + pos.height,
                pos.x, pos.y + pos.height,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::TrapezoidAlt => {
            let offset = 15.0_f32.min(pos.width / 4.0);
            svg.push_str(&format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                pos.x, pos.y,
                pos.x + pos.width, pos.y,
                pos.x + pos.width - offset, pos.y + pos.height,
                pos.x + offset, pos.y + pos.height,
                style.node_fill, style.node_stroke
            ));
        }
    }

    // Draw label
    let text_x = pos.x + pos.width / 2.0;
    let text_y = pos.y + pos.height / 2.0;

    // Handle multi-line labels
    let line_height = style.font_size * 1.2;
    let total_height = line_height * lines.len() as f32;
    let start_y = text_y - (total_height / 2.0) + line_height / 2.0;

    for (i, line) in lines.iter().enumerate() {
        let y = start_y + i as f32 * line_height;
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" dy="0.35em" font-family="{}" font-size="{:.1}" font-weight="500" fill="{}" text-anchor="middle">{}</text>"#,
            text_x, y, style.font_family, style.font_size, style.node_text, escape_xml(line)
        ));
    }

    svg
}

/// Clip a point from center of a node to its shape boundary.
fn clip_to_shape(
    node: &super::types::FlowchartNode,
    pos: &LayoutPos,
    target_x: f32,
    target_y: f32,
) -> (f32, f32) {
    let cx = pos.x + pos.width / 2.0;
    let cy = pos.y + pos.height / 2.0;
    let dx = target_x - cx;
    let dy = target_y - cy;
    if dx.abs() < 0.001 && dy.abs() < 0.001 {
        return (cx, cy);
    }

    match node.shape {
        NodeShape::Circle | NodeShape::DoubleCircle => {
            let r = pos.width.min(pos.height) / 2.0;
            let dist = (dx * dx + dy * dy).sqrt();
            (cx + dx / dist * r, cy + dy / dist * r)
        }
        NodeShape::Rhombus => {
            let hw = pos.width / 2.0;
            let hh = pos.height / 2.0;
            let t = 1.0 / (dx.abs() / hw + dy.abs() / hh);
            (cx + dx * t, cy + dy * t)
        }
        _ => {
            let hw = pos.width / 2.0;
            let hh = pos.height / 2.0;
            let scale_x = if dx.abs() > 0.001 {
                hw / dx.abs()
            } else {
                f32::MAX
            };
            let scale_y = if dy.abs() > 0.001 {
                hh / dy.abs()
            } else {
                f32::MAX
            };
            let scale = scale_x.min(scale_y);
            (cx + dx * scale, cy + dy * scale)
        }
    }
}

struct RenderEdgeContext<'a, T: TextMeasure> {
    edge: &'a super::types::FlowchartEdge,
    from_node: &'a super::types::FlowchartNode,
    from: &'a LayoutPos,
    to_node: &'a super::types::FlowchartNode,
    to: &'a LayoutPos,
    style: &'a DiagramStyle,
    direction: &'a FlowDirection,
    waypoints: &'a [(f32, f32)],
    measure: &'a mut T,
    route_index: usize,
    route_total: usize,
    outer_lane: Option<usize>,
    loop_label_width: f32,
    extents: &'a mut Vec<Rect>,
    group_bounds: &'a [Rect],
    occupied_labels: &'a mut Vec<Rect>,
    node_bounds: &'a [Rect],
    labels: &'a mut String,
}

fn render_edge<T: TextMeasure>(ctx: &mut RenderEdgeContext<'_, T>) -> String {
    let edge = ctx.edge;
    let from_node = ctx.from_node;
    let from = ctx.from;
    let to_node = ctx.to_node;
    let to = ctx.to;
    let style = ctx.style;
    let direction = ctx.direction;
    let waypoints = ctx.waypoints;

    let mut svg = String::new();
    let vertical = matches!(direction, FlowDirection::TopDown | FlowDirection::BottomUp);

    let (dash_attr, stroke_width) = match edge.style {
        EdgeStyle::Solid => ("", 0.75),
        EdgeStyle::Dotted => (" stroke-dasharray=\"4,4\"", 0.75),
        EdgeStyle::Thick => ("", 1.5),
    };

    let from_cx = from.x + from.width / 2.0;
    let from_cy = from.y + from.height / 2.0;
    let to_cx = to.x + to.width / 2.0;
    let to_cy = to.y + to.height / 2.0;

    // Self-loop: render a curved loopback path.
    // Adjust loop direction based on the graph flow direction so the loop
    // doesn't overlap content in the primary flow axis.
    if edge.from == edge.to {
        // The cubic midpoint reaches 3/4 of the control-point excursion.
        // TD loops must leave enough room for a label beside the node and for
        // successive labels along the nested loops.
        let loop_radius = if vertical {
            (ctx.loop_label_width / 2.0 + 28.0) / 0.75
                + ctx.route_index as f32 * (ctx.loop_label_width + 48.0) / 0.75
        } else {
            48.0 + ctx.route_index as f32 * 52.0
        };
        let port_fraction = 0.2 + 0.15 * ctx.route_index as f32 / ctx.route_total as f32;

        let (exit_x, exit_y, enter_x, enter_y, cp1_x, cp1_y, cp2_x, cp2_y) = match direction {
            FlowDirection::LeftRight | FlowDirection::RightLeft => {
                // Horizontal graph: loop upward
                let exit_x = from_cx + from.width * port_fraction;
                let exit_y = from.y;
                let enter_x = from_cx - from.width * port_fraction;
                let enter_y = from.y;
                let cp1_x = exit_x + loop_radius;
                let cp1_y = exit_y - loop_radius * 1.5;
                let cp2_x = enter_x - loop_radius;
                let cp2_y = enter_y - loop_radius * 1.5;
                (exit_x, exit_y, enter_x, enter_y, cp1_x, cp1_y, cp2_x, cp2_y)
            }
            // TopDown, BottomUp: loop to the right
            _ => {
                let exit_x = from.x + from.width;
                let exit_y = from_cy - from.height * port_fraction;
                let enter_x = from.right();
                let enter_y = from_cy + from.height * port_fraction;
                let cp1_x = exit_x + loop_radius;
                let cp1_y = exit_y - loop_radius * 0.5;
                let cp2_x = enter_x + loop_radius;
                let cp2_y = enter_y + loop_radius * 0.5;
                (exit_x, exit_y, enter_x, enter_y, cp1_x, cp1_y, cp2_x, cp2_y)
            }
        };
        let (exit_x, exit_y) = clip_to_shape(from_node, from, exit_x, exit_y);
        let (enter_x, enter_y) = clip_to_shape(to_node, to, enter_x, enter_y);

        svg.push_str(&format!(
            r#"<path d="M {:.2},{:.2} C {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="none" stroke="{}" stroke-width="{:.2}"{} />"#,
            exit_x, exit_y,
            cp1_x, cp1_y,
            cp2_x, cp2_y,
            enter_x, enter_y,
            style.edge_stroke, stroke_width, dash_attr
        ));

        // Follow the final curve tangent into the actual shape boundary.
        if edge.arrow_head != ArrowType::None {
            svg.push_str(&render_arrow_head(
                enter_x,
                enter_y,
                (enter_y - cp2_y).atan2(enter_x - cp2_x),
                &edge.arrow_head,
                style,
            ));
        }
        if edge.arrow_tail != ArrowType::None {
            svg.push_str(&render_arrow_head(
                exit_x,
                exit_y,
                (exit_y - cp1_y).atan2(exit_x - cp1_x),
                &edge.arrow_tail,
                style,
            ));
        }
        let min_x = exit_x.min(enter_x).min(cp1_x).min(cp2_x);
        let min_y = exit_y.min(enter_y).min(cp1_y).min(cp2_y);
        ctx.extents.push(Rect::new(
            min_x - 8.0,
            min_y - 8.0,
            exit_x.max(enter_x).max(cp1_x).max(cp2_x) - min_x + 16.0,
            exit_y.max(enter_y).max(cp1_y).max(cp2_y) - min_y + 16.0,
        ));

        // Keep the label just outside the curve's crown, rather than obscuring
        // the loop, and apply the same collision rules used by ordinary edges.
        let mut midpoint = (
            (exit_x + 3.0 * cp1_x + 3.0 * cp2_x + enter_x) / 8.0,
            (exit_y + 3.0 * cp1_y + 3.0 * cp2_y + enter_y) / 8.0,
        );
        if vertical {
            midpoint.0 += ctx.loop_label_width / 2.0 + 16.0;
        } else {
            midpoint.1 -= (style.font_size * 0.85 + 16.0) / 2.0 + 8.0;
        }
        render_edge_label(ctx, &[midpoint]);

        return svg;
    }

    // Determine exit and entry ports based on flow direction and relative position
    let (exit_x, exit_y, enter_x, enter_y) = if vertical {
        let going_down = from_cy <= to_cy;
        let ey = if going_down { from.bottom() } else { from.y };
        let ny = if going_down { to.y } else { to.bottom() };
        // Handle same-rank (same y) case: route horizontally
        if (from_cy - to_cy).abs() < 1.0 {
            let going_right = from_cx < to_cx;
            let ex = if going_right { from.right() } else { from.x };
            let nx = if going_right { to.x } else { to.right() };
            (ex, from_cy, nx, to_cy)
        } else {
            (from_cx, ey, to_cx, ny)
        }
    } else {
        let going_right = from_cx <= to_cx;
        let ex = if going_right { from.right() } else { from.x };
        let nx = if going_right { to.x } else { to.right() };
        // Handle same-column case: route vertically
        if (from_cx - to_cx).abs() < 1.0 {
            let going_down = from_cy < to_cy;
            let ey = if going_down { from.bottom() } else { from.y };
            let ny = if going_down { to.y } else { to.bottom() };
            (from_cx, ey, to_cx, ny)
        } else {
            (ex, from_cy, nx, to_cy)
        }
    };

    let (exit_x, exit_y, enter_x, enter_y) = if ctx.outer_lane.is_some() {
        // Keep forward relations on their ordinary route; return/parallel lanes use
        // the opposite side from self-loops. Bound ports to the node's side.
        let fraction = if ctx.route_total > 1 {
            ctx.route_index as f32 / ctx.route_total as f32 - 0.5
        } else {
            0.0
        };
        if vertical {
            (
                from.x,
                from_cy + fraction * from.height * 0.6,
                to.x,
                to_cy + fraction * to.height * 0.6,
            )
        } else {
            (
                from_cx + fraction * from.width * 0.6,
                from.bottom(),
                to_cx + fraction * to.width * 0.6,
                to.bottom(),
            )
        }
    } else {
        (exit_x, exit_y, enter_x, enter_y)
    };

    // Clip exit/enter points to actual shape boundaries
    let (exit_x, exit_y) = clip_to_shape(from_node, from, exit_x, exit_y);
    let (enter_x, enter_y) = clip_to_shape(to_node, to, enter_x, enter_y);

    // Build polyline points through waypoints
    let mut all_x = vec![exit_x];
    let mut all_y = vec![exit_y];
    for &(wx, wy) in waypoints {
        all_x.push(wx);
        all_y.push(wy);
    }
    all_x.push(enter_x);
    all_y.push(enter_y);

    let mut points: Vec<(f32, f32)> = Vec::new();
    points.push((all_x[0], all_y[0]));

    for i in 0..all_x.len() - 1 {
        let x1 = all_x[i];
        let y1 = all_y[i];
        let x2 = all_x[i + 1];
        let y2 = all_y[i + 1];

        if vertical || ((from_cy - to_cy).abs() < 1.0 && waypoints.is_empty()) {
            // Vertical primary axis (or horizontal same-rank): dogleg with vertical-first
            if (x1 - x2).abs() > 0.5 {
                let mid_y = (y1 + y2) / 2.0;
                points.push((x1, mid_y));
                points.push((x2, mid_y));
            }
        } else {
            // Horizontal primary axis: dogleg with horizontal-first
            if (y1 - y2).abs() > 0.5 {
                let mid_x = (x1 + x2) / 2.0;
                points.push((mid_x, y1));
                points.push((mid_x, y2));
            }
        }
        points.push((x2, y2));
    }

    if let Some(lane_index) = ctx.outer_lane {
        points = if vertical {
            let lane = ctx
                .node_bounds
                .iter()
                .map(|r| r.x)
                .fold(from.x.min(to.x), f32::min)
                - 30.0
                - lane_index as f32 * 40.0;
            vec![
                (exit_x, exit_y),
                (lane, exit_y),
                (lane, enter_y),
                (enter_x, enter_y),
            ]
        } else {
            let lane = ctx
                .node_bounds
                .iter()
                .map(|r| r.y + r.h)
                .fold(from.bottom().max(to.bottom()), f32::max)
                + 30.0
                + lane_index as f32 * 40.0;
            vec![
                (exit_x, exit_y),
                (exit_x, lane),
                (enter_x, lane),
                (enter_x, enter_y),
            ]
        };
    }
    // Coincident dogleg vertices have no direction and must not determine markers.
    points.dedup_by(|a, b| (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01);
    for &(x, y) in &points {
        ctx.extents.push(Rect::new(x - 8.0, y - 8.0, 16.0, 16.0));
    }

    // Render polyline
    let points_str: String = points
        .iter()
        .map(|(x, y)| format!("{:.2},{:.2}", x, y))
        .collect::<Vec<_>>()
        .join(" ");
    svg.push_str(&format!(
        r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{:.2}" stroke-linecap="round" stroke-linejoin="round"{} />"#,
        points_str, style.edge_stroke, stroke_width, dash_attr
    ));

    // Arrow head aligned to final segment
    let head_angle = if points.len() >= 2 {
        let last = points[points.len() - 1];
        let prev = points[points.len() - 2];
        (last.1 - prev.1).atan2(last.0 - prev.0)
    } else {
        (enter_y - exit_y).atan2(enter_x - exit_x)
    };

    if edge.arrow_head != ArrowType::None {
        svg.push_str(&render_arrow_head(
            enter_x,
            enter_y,
            head_angle,
            &edge.arrow_head,
            style,
        ));
    }

    // Arrow tail
    if edge.arrow_tail != ArrowType::None {
        let tail_angle = if points.len() >= 2 {
            let first = points[0];
            let second = points[1];
            (first.1 - second.1).atan2(first.0 - second.0)
        } else {
            head_angle + std::f32::consts::PI
        };
        svg.push_str(&render_arrow_head(
            exit_x,
            exit_y,
            tail_angle,
            &edge.arrow_tail,
            style,
        ));
    }

    render_edge_label(ctx, &points);
    svg
}

fn render_edge_label<T: TextMeasure>(ctx: &mut RenderEdgeContext<'_, T>, points: &[(f32, f32)]) {
    let edge = ctx.edge;
    let style = ctx.style;
    let measure = &mut *ctx.measure;
    // Label on middle segment (or the midpoint of a self-loop).
    let mid = points.len() / 2;
    let mut label_x = if points.len() >= 2 {
        (points[mid.saturating_sub(1)].0 + points[mid.min(points.len() - 1)].0) / 2.0
    } else {
        points[0].0
    };
    let mut label_y = if points.len() >= 2 {
        (points[mid.saturating_sub(1)].1 + points[mid.min(points.len() - 1)].1) / 2.0
    } else {
        points[0].1
    };

    if let Some(ref label) = edge.label {
        let cleaned = crate::xml::sanitize_xml_text(label);
        let label_font_size = style.font_size * 0.85;
        let text_w = measure
            .measure_text(&cleaned, label_font_size, false, false, false, None)
            .0;
        let pill_pad = 8.0;
        let pill_w = (text_w + pill_pad * 2.0).max(label_font_size * 2.5);
        let pill_h = label_font_size + pill_pad * 2.0;
        if points.len() >= 2 {
            let a = points[mid.saturating_sub(1)];
            let b = points[mid.min(points.len() - 1)];
            if (a.1 - b.1).abs() < 0.1 && (a.0 - b.0).abs() < pill_w + 32.0 {
                label_y -= pill_h / 2.0 + 8.0;
            } else if (a.0 - b.0).abs() < 0.1 && (a.1 - b.1).abs() < pill_h + 32.0 {
                label_x += pill_w / 2.0 + 8.0;
            }
        }
        // Keep labels off the horizontal borders of groups crossed by the route.
        for group in ctx.group_bounds {
            if label_x + pill_w / 2.0 > group.x && label_x - pill_w / 2.0 < group.x + group.w {
                for border in [group.y, group.y + group.h] {
                    if (label_y - border).abs() < pill_h / 2.0 + 4.0 {
                        label_y = if label_y >= border {
                            border + pill_h / 2.0 + 6.0
                        } else {
                            border - pill_h / 2.0 - 6.0
                        };
                    }
                }
            }
        }
        let mut label_bounds = Rect::new(
            label_x - pill_w / 2.0,
            label_y - pill_h / 2.0,
            pill_w,
            pill_h,
        );
        let clear = |candidate: Rect| {
            !ctx.occupied_labels
                .iter()
                .any(|other| candidate.with_padding(4.0).overlaps(other))
                && !ctx.node_bounds.iter().any(|node| candidate.overlaps(node))
                && !ctx.group_bounds.iter().any(|group| {
                    [
                        Rect::new(group.x, group.y, group.w, 0.0),
                        Rect::new(group.x, group.y + group.h, group.w, 0.0),
                        Rect::new(group.x, group.y, 0.0, group.h),
                        Rect::new(group.x + group.w, group.y, 0.0, group.h),
                    ]
                    .iter()
                    .any(|border| candidate.overlaps(&border.with_padding(4.0)))
                })
        };
        if !clear(label_bounds) {
            // Prefer moving along the label's route segment, keeping it nearby.
            // Expanding the search also lets dense branches escape node/group borders.
            let horizontal = points.len() < 2 || (points[mid - 1].1 - points[mid].1).abs() < 0.1;
            let mut radius = 1.0;
            'search: loop {
                let dx = (pill_w + 8.0) * radius;
                let dy = (pill_h + 8.0) * radius;
                let mut offsets = [(dx, 0.0), (-dx, 0.0), (0.0, dy), (0.0, -dy)];
                if !horizontal {
                    offsets.rotate_left(2);
                }
                for (dx, dy) in offsets {
                    let candidate =
                        Rect::new(label_bounds.x + dx, label_bounds.y + dy, pill_w, pill_h);
                    if clear(candidate) {
                        label_x += dx;
                        label_y += dy;
                        label_bounds = candidate;
                        break 'search;
                    }
                }
                radius += 1.0;
            }
        }
        ctx.extents.push(label_bounds);
        ctx.occupied_labels.push(label_bounds);

        ctx.labels.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="4" fill="{}" stroke="{}" stroke-width="0.5" />"#,
            label_x - pill_w / 2.0,
            label_y - pill_h / 2.0,
            pill_w,
            pill_h,
            style.node_fill,
            style.node_stroke
        ));

        ctx.labels.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" dy="0.35em" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            label_x,
            label_y,
            style.font_family,
            label_font_size,
            style.edge_text,
            escape_xml(&cleaned)
        ));
    }
}

fn render_arrow_head(
    x: f32,
    y: f32,
    angle: f32,
    arrow_type: &ArrowType,
    style: &DiagramStyle,
) -> String {
    let cos = angle.cos();
    let sin = angle.sin();

    match arrow_type {
        ArrowType::Arrow => {
            let p1 = (x - cos * 8.0 + sin * 4.8, y - sin * 8.0 - cos * 4.8);
            let p2 = (x - cos * 8.0 - sin * 4.8, y - sin * 8.0 + cos * 4.8);
            format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" />"#,
                x, y, p1.0, p1.1, p2.0, p2.1, style.edge_stroke
            )
        }
        ArrowType::Circle => {
            format!(
                r#"<circle cx="{:.2}" cy="{:.2}" r="5" fill="{}" stroke="{}" stroke-width="1" />"#,
                x - cos * 5.0,
                y - sin * 5.0,
                style.node_fill,
                style.edge_stroke
            )
        }
        ArrowType::Cross => {
            let s = 7.0_f32;
            let cx = x - cos * s;
            let cy = y - sin * s;
            // Rotate ±45° from the edge direction for an "×" shape
            let angle_a = angle + std::f32::consts::FRAC_PI_4;
            let angle_b = angle - std::f32::consts::FRAC_PI_4;
            let (ca, sa) = (angle_a.cos(), angle_a.sin());
            let (cb, sb) = (angle_b.cos(), angle_b.sin());
            format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2.5" /><line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2.5" />"#,
                cx + ca * s,
                cy + sa * s,
                cx - ca * s,
                cy - sa * s,
                style.edge_stroke,
                cx + cb * s,
                cy + sb * s,
                cx - cb * s,
                cy - sb * s,
                style.edge_stroke
            )
        }
        ArrowType::None => String::new(),
    }
}

fn subgraph_bbox(
    subgraph: &super::types::Subgraph,
    positions: &HashMap<String, LayoutPos>,
) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_right = f32::MIN;
    let mut max_bottom = f32::MIN;
    let mut found = false;

    for node_id in &subgraph.nodes {
        if let Some(pos) = positions.get(node_id) {
            found = true;
            min_x = min_x.min(pos.x);
            min_y = min_y.min(pos.y);
            max_right = max_right.max(pos.right());
            max_bottom = max_bottom.max(pos.bottom());
        }
    }

    if !found {
        return None;
    }

    let content_bbox = BBox::new(min_x, min_y, max_right - min_x, max_bottom - min_y);
    let padded_bbox = content_bbox.with_padding(20.0);
    let raw_y = padded_bbox.y - 20.0;
    let clamped_y = raw_y.max(0.0);
    Some((
        padded_bbox.x,
        clamped_y,
        padded_bbox.width,
        padded_bbox.height + 20.0 + (raw_y - clamped_y).abs(),
    ))
}

fn render_subgraph_box(
    subgraph: &super::types::Subgraph,
    positions: &HashMap<String, LayoutPos>,
    style: &DiagramStyle,
) -> String {
    let Some((min_x, min_y, width, height)) = subgraph_bbox(subgraph, positions) else {
        return String::new();
    };

    format!(
        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="8" fill="{}" fill-opacity="0.3" stroke="{}" stroke-width="1" stroke-dasharray="4,2" />"#,
        min_x, min_y, width, height, style.node_fill, style.node_stroke
    )
}

fn render_subgraph_title(
    subgraph: &super::types::Subgraph,
    positions: &HashMap<String, LayoutPos>,
    style: &DiagramStyle,
    used_rects: &mut Vec<Rect>,
) -> String {
    if subgraph.title.is_empty() {
        return String::new();
    }

    let Some((min_x, min_y, _width, _height)) = subgraph_bbox(subgraph, positions) else {
        return String::new();
    };

    let title_font = style.font_size * 0.9;
    let approx_char_w = title_font * 0.6;
    let title_w = subgraph.title.len() as f32 * approx_char_w + 12.0;
    let title_h = title_font + 6.0;

    let title_x = min_x + 12.0;
    let mut title_y = min_y + title_font + 4.0;

    // Offset title if it would overlap a previously placed title
    for used in used_rects.iter() {
        let candidate = Rect::new(
            title_x - 4.0,
            title_y - title_h / 2.0 - 1.0,
            title_w,
            title_h,
        );
        if candidate.overlaps(used) {
            title_y = used.y + used.h + title_h / 2.0 + 3.0;
        }
    }

    let pill_x = title_x - 4.0;
    let pill_y = title_y - title_h / 2.0 - 1.0;
    used_rects.push(Rect::new(pill_x, pill_y, title_w, title_h));

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="3" fill="{}" fill-opacity="0.9" />"#,
        pill_x, pill_y, title_w, title_h, style.node_fill
    ));
    svg.push_str(&format!(
        r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" font-weight="bold" text-anchor="start">{}</text>"#,
        title_x, title_y, style.font_family, title_font, style.node_text, escape_xml(&subgraph.title)
    ));

    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockMeasure;

    impl TextMeasure for MockMeasure {
        fn measure_text(
            &mut self,
            text: &str,
            font_size: f32,
            _is_code: bool,
            _is_bold: bool,
            _is_italic: bool,
            _max_width: Option<f32>,
        ) -> (f32, f32) {
            (text.chars().count() as f32 * font_size * 0.6, font_size)
        }
    }

    #[test]
    fn long_node_labels_wrap_without_losing_unicode_or_explicit_breaks() {
        for label in [
            "Validate the customer request before saving the complete account information"
                .to_string(),
            "確認申請資料並且保存完整客戶帳戶資訊以便稍後審核和處理所有後續服務請求".to_string(),
            "e\u{301}".repeat(40),
            "👩‍💻".repeat(40),
            "First<br><br>Second<br/>Third<br />Fourth".to_string(),
        ] {
            let source = format!("flowchart TD\nA[\"{label}\"]");
            let style = DiagramStyle::default();
            let (svg, width, height) =
                crate::mermaid::render_diagram(&source, &style, &mut MockMeasure).unwrap();
            let mut reader = quick_xml::Reader::from_str(&svg);
            let mut lines = Vec::new();
            loop {
                match reader.read_event().unwrap() {
                    quick_xml::events::Event::Text(text) => {
                        lines.push(text.decode().unwrap().into_owned())
                    }
                    quick_xml::events::Event::Eof => break,
                    _ => {}
                }
            }
            assert!(lines.len() > 1, "{svg}");
            let original = label
                .replace("<br>", "")
                .replace("<br/>", "")
                .replace("<br />", "");
            assert_eq!(lines.concat().replace(' ', ""), original.replace(' ', ""));
            for line in &lines {
                assert!(!line.starts_with('\u{301}'), "split a combining grapheme");
                if label.starts_with('👩') {
                    use unicode_segmentation::UnicodeSegmentation;
                    assert!(line.graphemes(true).all(|grapheme| grapheme == "👩‍💻"));
                }
                assert!(
                    MockMeasure
                        .measure_text(line, style.font_size, false, false, false, None)
                        .0
                        <= style.font_size * 18.0
                );
            }
            assert!(width < 330.0, "wrapped node should remain compact: {width}");
            assert!(height >= lines.len() as f32 * style.font_size * 1.2 + 24.0);
            if label.starts_with("First") {
                assert_eq!(
                    svg.matches("<text ").count(),
                    5,
                    "preserve explicit blank line"
                );
            }
        }
    }

    fn first_polygon_points(svg: &str) -> Vec<(f32, f32)> {
        let marker = "<polygon points=\"";
        let start = svg.find(marker).expect("expected polygon");
        let rest = &svg[start + marker.len()..];
        let end = rest.find('"').expect("expected polygon points close quote");
        rest[..end]
            .split_whitespace()
            .map(|pair| {
                let mut it = pair.split(',');
                let x = it
                    .next()
                    .expect("x")
                    .parse::<f32>()
                    .expect("x should parse");
                let y = it
                    .next()
                    .expect("y")
                    .parse::<f32>()
                    .expect("y should parse");
                (x, y)
            })
            .collect()
    }

    #[test]
    fn orthogonal_edge_arrow_points_in_correct_direction() {
        let mut measure = MockMeasure;
        let style = DiagramStyle::default();
        let edge = super::super::types::FlowchartEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            style: EdgeStyle::Solid,
            arrow_head: ArrowType::Arrow,
            arrow_tail: ArrowType::None,
            min_length: 1,
        };
        let from_node = super::super::types::FlowchartNode {
            id: "A".to_string(),
            label: "A".to_string(),
            shape: NodeShape::Rect,
        };
        let to_node = super::super::types::FlowchartNode {
            id: "B".to_string(),
            label: "B".to_string(),
            shape: NodeShape::Rect,
        };
        let from = LayoutPos::new(0.0, 0.0, 100.0, 40.0);
        let to = LayoutPos::new(200.0, 100.0, 100.0, 40.0);

        let svg = render_edge(&mut RenderEdgeContext {
            edge: &edge,
            from_node: &from_node,
            from: &from,
            to_node: &to_node,
            to: &to,
            style: &style,
            direction: &FlowDirection::LeftRight,
            waypoints: &[],
            measure: &mut measure,
            route_index: 0,
            route_total: 1,
            outer_lane: None,
            loop_label_width: 0.0,
            extents: &mut Vec::new(),
            group_bounds: &[],
            occupied_labels: &mut Vec::new(),
            node_bounds: &[],
            labels: &mut String::new(),
        });

        let pts = first_polygon_points(&svg);
        assert_eq!(pts.len(), 3);

        // Orthogonal routing: exit right of from=(100,20), enter left of to=(200,120)
        // Arrow tip at entry point (200, 120), pointing right (angle=0)
        assert!((pts[0].0 - 200.0).abs() < 1.0, "tip x={}", pts[0].0);
        assert!((pts[0].1 - 120.0).abs() < 1.0, "tip y={}", pts[0].1);
    }

    #[test]
    fn test_self_loop_renders_visible_curve() {
        let source = "flowchart LR\n    A[Node A] --> A";
        let diagram = match crate::mermaid::parse_mermaid(source).unwrap() {
            crate::mermaid::MermaidDiagram::Flowchart(fc) => fc,
            _ => panic!("Expected flowchart"),
        };
        let style = DiagramStyle::default();
        let mut measure = MockMeasure;
        let (svg, _w, _h) = render_flowchart(&diagram, &style, &mut measure).unwrap();

        assert!(
            svg.contains("<path"),
            "Self-loop should render as a curve (path element), got:\n{}",
            svg
        );
    }

    #[test]
    fn test_self_loop_has_arrowhead() {
        let source = "flowchart TD\n    A --> A";
        let diagram = match crate::mermaid::parse_mermaid(source).unwrap() {
            crate::mermaid::MermaidDiagram::Flowchart(fc) => fc,
            _ => panic!("Expected flowchart"),
        };
        let style = DiagramStyle::default();
        let mut measure = MockMeasure;
        let (svg, _w, _h) = render_flowchart(&diagram, &style, &mut measure).unwrap();

        assert!(
            svg.contains("<polygon"),
            "Self-loop should have an arrowhead"
        );
    }

    #[test]
    fn crowded_loop_and_cycle_labels_clear_nodes_and_each_other() {
        for direction in ["LR", "TD", "RL", "BT"] {
            let source = format!(
                "flowchart {direction}\nA[Queued] -->|start| B{{Running}}\n\
                 B -->|retry| A\nA -->|resume| B\n\
                 B -->|heartbeat| B\nB -->|watchdog| B\n\
                 B -->|done| C((Finished))\nC -->|refresh| C\nC -->|revise| A"
            );
            let crate::mermaid::MermaidDiagram::Flowchart(diagram) =
                crate::mermaid::parse_mermaid(&source).unwrap()
            else {
                panic!("Expected flowchart");
            };
            let style = DiagramStyle::default();
            let (positions, _, _) =
                LayoutEngine::new(&mut MockMeasure, style.font_size).layout_flowchart(&diagram);
            let (svg, _, _) = render_flowchart(&diagram, &style, &mut MockMeasure).unwrap();
            let mut reader = quick_xml::Reader::from_str(&svg);
            let mut labels: Vec<Rect> = Vec::new();
            let mut routes = Vec::new();
            let mut curves: Vec<Vec<f32>> = Vec::new();
            while let Ok(event) = reader.read_event() {
                match event {
                    quick_xml::events::Event::Empty(element) => {
                        let attrs: HashMap<String, String> = element
                            .attributes()
                            .map(|a| {
                                let a = a.unwrap();
                                (
                                    String::from_utf8(a.key.as_ref().to_vec()).unwrap(),
                                    String::from_utf8(a.value.to_vec()).unwrap(),
                                )
                            })
                            .collect();
                        if element.name().as_ref() == b"rect"
                            && attrs.get("rx").map(String::as_str) == Some("4")
                        {
                            let value = |key: &str| attrs[key].parse::<f32>().unwrap();
                            let rect =
                                Rect::new(value("x"), value("y"), value("width"), value("height"));
                            assert!(
                                labels.iter().all(|other| !rect.overlaps(other)),
                                "{direction}: overlapping labels"
                            );
                            assert!(
                                positions.values().all(
                                    |p| !rect.overlaps(&Rect::new(p.x, p.y, p.width, p.height))
                                ),
                                "{direction}: label over node"
                            );
                            labels.push(rect);
                        }
                        if element.name().as_ref() == b"polyline" {
                            routes.push(attrs["points"].clone());
                        }
                        if element.name().as_ref() == b"path"
                            && attrs.get("d").is_some_and(|d| d.contains(" C "))
                        {
                            curves.push(
                                attrs["d"]
                                    .split([' ', ','])
                                    .filter_map(|s| s.parse().ok())
                                    .collect(),
                            );
                        }
                    }
                    quick_xml::events::Event::Eof => break,
                    _ => {}
                }
            }
            assert_eq!(labels.len(), 8);
            assert_eq!(routes.len(), 5);
            for route in &routes {
                let points: Vec<(f32, f32)> = route
                    .split_whitespace()
                    .map(|pair| {
                        let (x, y) = pair.split_once(',').unwrap();
                        (x.parse().unwrap(), y.parse().unwrap())
                    })
                    .collect();
                for segment in points.windows(2) {
                    let (a, b) = (segment[0], segment[1]);
                    for node in positions.values() {
                        let (x, y) = node.center();
                        let crosses = if (a.0 - b.0).abs() < 0.1 {
                            (x - a.0).abs() < 1.0 && y > a.1.min(b.1) && y < a.1.max(b.1)
                        } else {
                            (y - a.1).abs() < 1.0 && x > a.0.min(b.0) && x < a.0.max(b.0)
                        };
                        assert!(!crosses, "{direction}: route passes through a node center");
                    }
                }
            }
            for curve in curves {
                for i in 1..100 {
                    let t = i as f32 / 100.0;
                    let u = 1.0 - t;
                    let coordinate = |axis| {
                        u * u * u * curve[axis]
                            + 3.0 * u * u * t * curve[axis + 2]
                            + 3.0 * u * t * t * curve[axis + 4]
                            + t * t * t * curve[axis + 6]
                    };
                    let sample = Rect::new(coordinate(0), coordinate(1), 0.01, 0.01);
                    assert!(
                        labels.iter().all(|label| !sample.overlaps(label)),
                        "{direction}: label obscures self-loop"
                    );
                }
            }
            let mut distinct = routes.clone();
            distinct.sort();
            distinct.dedup();
            assert_eq!(
                distinct.len(),
                routes.len(),
                "{direction}: coincident relations"
            );
        }
    }
}

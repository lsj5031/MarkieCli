use std::collections::HashMap;

use super::layout::{BBox, LayoutEngine, LayoutPos};
use super::render::{DiagramStyle, escape_xml};
use super::types::{ArrowType, EdgeStyle, FlowDirection, Flowchart, NodeShape};

/// Render a flowchart to SVG
pub fn render_flowchart(
    flowchart: &Flowchart,
    style: &DiagramStyle,
) -> Result<(String, f32, f32), String> {
    if flowchart.nodes.is_empty() {
        return Ok(("<g></g>".to_string(), 100.0, 50.0));
    }

    let layout = LayoutEngine::new();
    let (positions, bbox) = layout.layout_flowchart(flowchart);

    let mut svg = String::new();
    let padding = 20.0;

    // Draw edges first (behind nodes)
    for edge in &flowchart.edges {
        let from_pos = positions.get(&edge.from);
        let to_pos = positions.get(&edge.to);

        if let (Some(from), Some(to)) = (from_pos, to_pos) {
            svg.push_str(&render_edge(edge, from, to, style, &flowchart.direction));
        }
    }

    // Draw nodes
    for node in &flowchart.nodes {
        if let Some(pos) = positions.get(&node.id) {
            svg.push_str(&render_node(&node.label, &node.shape, pos, style));
        }
    }

    // Draw subgraphs
    for subgraph in &flowchart.subgraphs {
        svg.push_str(&render_subgraph(subgraph, &positions, style));
    }

    let total_width = bbox.right() + padding;
    let total_height = bbox.bottom() + padding;

    Ok((svg, total_width, total_height))
}

fn render_node(label: &str, shape: &NodeShape, pos: &LayoutPos, style: &DiagramStyle) -> String {
    let mut svg = String::new();
    let escaped_label = escape_xml(label);

    match shape {
        NodeShape::Rect => {
            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                pos.x, pos.y, pos.width, pos.height,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::RoundedRect => {
            let rx = 8.0_f32.min(pos.height / 4.0);
            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                pos.x, pos.y, pos.width, pos.height, rx,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Stadium => {
            let rx = pos.height / 2.0;
            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                pos.x, pos.y, pos.width, pos.height, rx,
                style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Subroutine => {
            // Rect with vertical lines at ends
            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
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
            // Body
            svg.push_str(&format!(
                r#"<path d="M {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2} Z" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                pos.x, pos.y + cap_height,
                pos.x + pos.width, pos.y + cap_height,
                pos.x + pos.width, pos.y + pos.height,
                pos.x, pos.y + pos.height,
                style.node_fill, style.node_stroke
            ));
            // Top ellipse
            svg.push_str(&format!(
                r#"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                pos.x + pos.width / 2.0, pos.y + cap_height,
                pos.width / 2.0, cap_height,
                style.node_fill, style.node_stroke
            ));
            // Bottom ellipse arc (visible part)
            svg.push_str(&format!(
                r#"<path d="M {:.2} {:.2} A {:.2} {:.2} 0 0 0 {:.2} {:.2}" fill="none" stroke="{}" stroke-width="1.5" />"#,
                pos.x + pos.width, pos.y + pos.height,
                pos.width / 2.0, cap_height,
                pos.x, pos.y + pos.height,
                style.node_stroke
            ));
        }
        NodeShape::Circle => {
            let radius = pos.width.min(pos.height) / 2.0;
            let cx = pos.x + pos.width / 2.0;
            let cy = pos.y + pos.height / 2.0;
            svg.push_str(&format!(
                r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                cx, cy, radius, style.node_fill, style.node_stroke
            ));
        }
        NodeShape::DoubleCircle => {
            let radius = pos.width.min(pos.height) / 2.0 - 4.0;
            let cx = pos.x + pos.width / 2.0;
            let cy = pos.y + pos.height / 2.0;
            svg.push_str(&format!(
                r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                cx, cy, radius + 4.0, style.node_fill, style.node_stroke
            ));
            svg.push_str(&format!(
                r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                cx, cy, radius, style.node_fill, style.node_stroke
            ));
        }
        NodeShape::Rhombus => {
            let cx = pos.x + pos.width / 2.0;
            let cy = pos.y + pos.height / 2.0;
            svg.push_str(&format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
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
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
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
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
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
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
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
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
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
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
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
    let text_y = pos.y + pos.height / 2.0 + style.font_size / 3.0;

    // Handle multi-line labels
    let lines: Vec<&str> = escaped_label.lines().collect();
    let line_height = style.font_size * 1.2;
    let total_height = line_height * lines.len() as f32;
    let start_y = text_y - (total_height / 2.0) + line_height / 2.0;

    for (i, line) in lines.iter().enumerate() {
        let y = start_y + i as f32 * line_height;
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            text_x, y, style.font_family, style.font_size, style.node_text, line
        ));
    }

    svg
}

fn render_edge(
    edge: &super::types::FlowchartEdge,
    from: &LayoutPos,
    to: &LayoutPos,
    style: &DiagramStyle,
    direction: &FlowDirection,
) -> String {
    let mut svg = String::new();

    let is_vertical = matches!(direction, FlowDirection::TopDown | FlowDirection::BottomUp);

    // Calculate connection points
    let (x1, y1, x2, y2) = if is_vertical {
        let from_cx = from.x + from.width / 2.0;
        let to_cx = to.x + to.width / 2.0;

        if to.y > from.y {
            // Downward
            (from_cx, from.y + from.height, to_cx, to.y)
        } else {
            // Upward
            (from_cx, from.y, to_cx, to.y + to.height)
        }
    } else {
        let from_cy = from.y + from.height / 2.0;
        let to_cy = to.y + to.height / 2.0;

        if to.x > from.x {
            // Rightward
            (from.x + from.width, from_cy, to.x, to_cy)
        } else {
            // Leftward
            (from.x, from_cy, to.x + to.width, to_cy)
        }
    };

    let (dash_attr, stroke_width) = match edge.style {
        EdgeStyle::Solid => ("", 1.5),
        EdgeStyle::Dotted => (" stroke-dasharray=\"4,4\"", 1.5),
        EdgeStyle::Thick => ("", 2.5),
    };

    // Draw line (straight or curved for better look)
    if (x1 - x2).abs() < 1.0 || (y1 - y2).abs() < 1.0 {
        // Straight line
        svg.push_str(&format!(
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{:.1}"{} />"#,
            x1, y1, x2, y2, style.edge_stroke, stroke_width, dash_attr
        ));
    } else {
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;

        let (cx1, cy1, cx2, cy2) = if is_vertical {
            (x1, my, x2, my)
        } else {
            (mx, y1, mx, y2)
        };

        svg.push_str(&format!(
            r#"<path d="M {:.2} {:.2} C {:.2} {:.2}, {:.2} {:.2}, {:.2} {:.2}" fill="none" stroke="{}" stroke-width="{:.1}"{} />"#,
            x1, y1, cx1, cy1, cx2, cy2, x2, y2, style.edge_stroke, stroke_width, dash_attr
        ));
    }

    // Arrow head
    let angle = (y2 - y1).atan2(x2 - x1);
    if edge.arrow_head != ArrowType::None {
        svg.push_str(&render_arrow_head(x2, y2, angle, &edge.arrow_head, style));
    }

    // Arrow tail (for bidirectional)
    if edge.arrow_tail != ArrowType::None {
        let tail_angle = angle + std::f32::consts::PI;
        svg.push_str(&render_arrow_head(
            x1,
            y1,
            tail_angle,
            &edge.arrow_tail,
            style,
        ));
    }

    // Edge label
    if let Some(ref label) = edge.label {
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;

        // Offset label slightly perpendicular to the edge
        let perp_x = -angle.sin() * 30.0;
        let perp_y = -angle.cos() * 30.0;

        let label_x = mx + perp_x;
        let label_y = my + perp_y;

        // Background rectangle for label
        let label_width = label.chars().count() as f32 * 7.0 + 8.0;
        let label_height = style.font_size + 6.0;

        svg.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" fill="{}" />"#,
            label_x - label_width / 2.0,
            label_y - label_height / 2.0,
            label_width,
            label_height,
            style.background
        ));

        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            label_x, label_y + style.font_size / 3.0, style.font_family, style.font_size * 0.85, style.edge_text, escape_xml(label)
        ));
    }

    svg
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
            let p1 = (x - cos * 12.0 + sin * 6.0, y - sin * 12.0 - cos * 6.0);
            let p2 = (x - cos * 12.0 - sin * 6.0, y - sin * 12.0 + cos * 6.0);
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
            let cx = x - cos * 8.0;
            let cy = y - sin * 8.0;
            let nc = -sin;
            let ns = cos;
            format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2" /><line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2" />"#,
                cx + nc * 5.0,
                cy + ns * 5.0,
                cx - nc * 5.0,
                cy - ns * 5.0,
                style.edge_stroke,
                cx + ns * 5.0,
                cy - nc * 5.0,
                cx - ns * 5.0,
                cy + nc * 5.0,
                style.edge_stroke
            )
        }
        ArrowType::None => String::new(),
    }
}

fn render_subgraph(
    subgraph: &super::types::Subgraph,
    positions: &HashMap<String, LayoutPos>,
    style: &DiagramStyle,
) -> String {
    let mut svg = String::new();

    // Find bounding box of all nodes in subgraph
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
        return svg;
    }

    let content_bbox = BBox::new(min_x, min_y, max_right - min_x, max_bottom - min_y);
    let padded_bbox = content_bbox.with_padding(15.0);
    let min_x = padded_bbox.x;
    let min_y = padded_bbox.y - 20.0;
    let width = padded_bbox.width;
    let height = padded_bbox.height + 20.0;
    let title_center_y = (min_y + padded_bbox.y) / 2.0;
    let title_x = padded_bbox.center_x();
    let _title_anchor_hint = padded_bbox.center_y().min(title_center_y);

    svg.push_str(&format!(r#"<g id="{}">"#, escape_xml(&subgraph.id)));

    // Draw subgraph box
    svg.push_str(&format!(
        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="8" fill="{}" fill-opacity="0.3" stroke="{}" stroke-width="1" stroke-dasharray="4,2" />"#,
        min_x, min_y, width, height,
        style.node_fill, style.node_stroke
    ));

    // Draw title
    if !subgraph.title.is_empty() {
        let title_y = title_center_y + style.font_size * 0.3;

        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" font-weight="bold" text-anchor="middle">{}</text>"#,
            title_x, title_y, style.font_family, style.font_size * 0.9, style.node_text, escape_xml(&subgraph.title)
        ));
    }

    svg.push_str("</g>");

    svg
}

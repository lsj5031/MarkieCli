use std::collections::HashMap;

use crate::fonts::TextMeasure;

use super::layout::{BBox, LayoutEngine, LayoutPos};
use super::render::{DiagramStyle, escape_xml};
use super::types::{ArrowType, EdgeStyle, FlowDirection, Flowchart, NodeShape};

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
    let (positions, bbox) = layout.layout_flowchart(flowchart);

    let mut svg = String::new();
    let padding = 20.0;

    let node_map: HashMap<&str, &super::types::FlowchartNode> =
        flowchart.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Draw edges first (behind nodes)
    for edge in &flowchart.edges {
        let from_pos = positions.get(&edge.from);
        let to_pos = positions.get(&edge.to);
        let from_node = node_map.get(edge.from.as_str());
        let to_node = node_map.get(edge.to.as_str());

        if let (Some(from), Some(to), Some(fn_), Some(tn)) =
            (from_pos, to_pos, from_node, to_node)
        {
            svg.push_str(&render_edge(
                edge,
                fn_,
                from,
                tn,
                to,
                style,
                &flowchart.direction,
                measure,
            ));
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
    let lines: Vec<&str> = escaped_label.lines().collect();
    let line_height = style.font_size * 1.2;
    let total_height = line_height * lines.len() as f32;
    let start_y = text_y - (total_height / 2.0) + line_height / 2.0;

    for (i, line) in lines.iter().enumerate() {
        let y = start_y + i as f32 * line_height;
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" dy="0.35em" font-family="{}" font-size="{:.1}" font-weight="500" fill="{}" text-anchor="middle">{}</text>"#,
            text_x, y, style.font_family, style.font_size, style.node_text, line
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
            // Diamond boundary: |dx|/hw + |dy|/hh = 1
            let t = 1.0 / (dx.abs() / hw + dy.abs() / hh);
            (cx + dx * t, cy + dy * t)
        }
        _ => {
            // Rectangle boundary clipping
            let hw = pos.width / 2.0;
            let hh = pos.height / 2.0;
            let scale_x = if dx.abs() > 0.001 { hw / dx.abs() } else { f32::MAX };
            let scale_y = if dy.abs() > 0.001 { hh / dy.abs() } else { f32::MAX };
            let scale = scale_x.min(scale_y);
            (cx + dx * scale, cy + dy * scale)
        }
    }
}

fn render_edge(
    edge: &super::types::FlowchartEdge,
    from_node: &super::types::FlowchartNode,
    from: &LayoutPos,
    to_node: &super::types::FlowchartNode,
    to: &LayoutPos,
    style: &DiagramStyle,
    direction: &FlowDirection,
    measure: &mut impl TextMeasure,
) -> String {
    let mut svg = String::new();

    let is_vertical = matches!(direction, FlowDirection::TopDown | FlowDirection::BottomUp);

    let from_cx = from.x + from.width / 2.0;
    let from_cy = from.y + from.height / 2.0;
    let to_cx = to.x + to.width / 2.0;
    let to_cy = to.y + to.height / 2.0;

    // Clip endpoints to actual shape boundaries
    let (x1, y1) = clip_to_shape(from_node, from, to_cx, to_cy);
    let (x2, y2) = clip_to_shape(to_node, to, from_cx, from_cy);

    let (dash_attr, stroke_width) = match edge.style {
        EdgeStyle::Solid => ("", 0.75),
        EdgeStyle::Dotted => (" stroke-dasharray=\"4,4\"", 0.75),
        EdgeStyle::Thick => ("", 1.5),
    };

    let head_angle;
    let tail_angle;

    // Orthogonal routing with Z-shaped (3-segment) paths to avoid crossing nodes
    let is_aligned = if is_vertical {
        (from_cx - to_cx).abs() < 1.0
    } else {
        (from_cy - to_cy).abs() < 1.0
    };

    // Label anchor: where to place the label pill
    let mut label_x = (x1 + x2) / 2.0;
    let mut label_y = (y1 + y2) / 2.0;

    if is_aligned {
        // Straight line
        let edge_angle = (y2 - y1).atan2(x2 - x1);
        head_angle = edge_angle;
        tail_angle = edge_angle + std::f32::consts::PI;
        svg.push_str(&format!(
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{:.2}"{} />"#,
            x1, y1, x2, y2, style.edge_stroke, stroke_width, dash_attr
        ));
    } else if is_vertical {
        // Z-shaped: vertical → horizontal → vertical
        // Bend at midpoint Y between source bottom and target top
        let mid_y = (y1 + y2) / 2.0;
        head_angle = std::f32::consts::FRAC_PI_2; // entering from top
        tail_angle = -std::f32::consts::FRAC_PI_2; // leaving from bottom

        svg.push_str(&format!(
            r#"<polyline points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="none" stroke="{}" stroke-width="{:.2}"{} />"#,
            x1, y1, x1, mid_y, x2, mid_y, x2, y2,
            style.edge_stroke, stroke_width, dash_attr
        ));

        // Place label on the horizontal segment
        label_x = (x1 + x2) / 2.0;
        label_y = mid_y;
    } else {
        // Z-shaped: horizontal → vertical → horizontal
        let mid_x = (x1 + x2) / 2.0;
        head_angle = if x2 > x1 { 0.0 } else { std::f32::consts::PI };
        tail_angle = head_angle + std::f32::consts::PI;

        svg.push_str(&format!(
            r#"<polyline points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="none" stroke="{}" stroke-width="{:.2}"{} />"#,
            x1, y1, mid_x, y1, mid_x, y2, x2, y2,
            style.edge_stroke, stroke_width, dash_attr
        ));

        // Place label on the vertical segment
        label_x = mid_x;
        label_y = (y1 + y2) / 2.0;
    }

    // Arrow head
    if edge.arrow_head != ArrowType::None {
        svg.push_str(&render_arrow_head(
            x2,
            y2,
            head_angle,
            &edge.arrow_head,
            style,
        ));
    }

    // Arrow tail (for bidirectional)
    if edge.arrow_tail != ArrowType::None {
        svg.push_str(&render_arrow_head(
            x1,
            y1,
            tail_angle,
            &edge.arrow_tail,
            style,
        ));
    }

    // Edge label with pill background
    if let Some(ref label) = edge.label {
        let cleaned = crate::xml::sanitize_xml_text(label);
        let label_font_size = style.font_size * 0.85;
        let text_w = measure
            .measure_text(&cleaned, label_font_size, false, false, false, None)
            .0;
        let pill_pad = 6.0;
        let pill_w = text_w + pill_pad * 2.0;
        let pill_h = label_font_size + pill_pad * 2.0;

        svg.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="4" fill="{}" stroke="{}" stroke-width="0.5" />"#,
            label_x - pill_w / 2.0,
            label_y - pill_h / 2.0,
            pill_w,
            pill_h,
            style.background,
            style.node_stroke
        ));

        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" dy="0.35em" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            label_x,
            label_y,
            style.font_family,
            label_font_size,
            style.edge_text,
            escape_xml(&cleaned)
        ));
    }

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

        let svg = render_edge(
            &edge,
            &from_node,
            &from,
            &to_node,
            &to,
            &style,
            &FlowDirection::LeftRight,
            &mut measure,
        );

        let pts = first_polygon_points(&svg);
        assert_eq!(pts.len(), 3);

        // With shape clipping: to center=(250,120), from center=(50,20).
        // Rect clip hits top edge at (210, 100).
        assert!((pts[0].0 - 210.0).abs() < 1.0, "tip x={}", pts[0].0);
        assert!((pts[0].1 - 100.0).abs() < 1.0, "tip y={}", pts[0].1);
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

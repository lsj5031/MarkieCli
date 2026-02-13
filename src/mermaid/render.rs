use std::collections::{HashMap, HashSet};

use super::layout::{LayoutEngine, LayoutPos};
use super::types::*;
use super::{MermaidDiagram, parse_mermaid};

/// Style configuration for diagram rendering
#[derive(Debug, Clone)]
pub struct DiagramStyle {
    pub node_fill: String,
    pub node_stroke: String,
    pub node_text: String,
    pub edge_stroke: String,
    pub edge_text: String,
    pub background: String,
    pub font_family: String,
    pub font_size: f32,
}

impl Default for DiagramStyle {
    fn default() -> Self {
        Self {
            node_fill: "#f5f5f5".to_string(),
            node_stroke: "#333333".to_string(),
            node_text: "#333333".to_string(),
            edge_stroke: "#333333".to_string(),
            edge_text: "#666666".to_string(),
            background: "transparent".to_string(),
            font_family: "sans-serif".to_string(),
            font_size: 14.0,
        }
    }
}

impl DiagramStyle {
    pub fn from_theme(text_color: &str, background: &str, code_bg: &str) -> Self {
        let diagram_fg = pick_higher_contrast(code_bg, text_color, background);
        let label_fg = pick_higher_contrast(background, text_color, code_bg);

        Self {
            node_fill: code_bg.to_string(),
            node_stroke: diagram_fg.clone(),
            node_text: diagram_fg.clone(),
            edge_stroke: diagram_fg,
            edge_text: label_fg,
            background: background.to_string(),
            font_family: "sans-serif".to_string(),
            font_size: 14.0,
        }
    }
}

fn parse_hex_rgb(value: &str) -> Option<(f32, f32, f32)> {
    let hex = value.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some((r, g, b))
}

fn relative_luminance(color: (f32, f32, f32)) -> f32 {
    let linear = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };

    let (r, g, b) = color;
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

fn contrast_ratio(a: &str, b: &str) -> Option<f32> {
    let l1 = relative_luminance(parse_hex_rgb(a)?);
    let l2 = relative_luminance(parse_hex_rgb(b)?);
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    Some((hi + 0.05) / (lo + 0.05))
}

fn pick_higher_contrast(base: &str, primary: &str, secondary: &str) -> String {
    let p = contrast_ratio(base, primary).unwrap_or(0.0);
    let s = contrast_ratio(base, secondary).unwrap_or(0.0);

    if s > p {
        secondary.to_string()
    } else {
        primary.to_string()
    }
}

/// Render any mermaid diagram to SVG
pub fn render_diagram(source: &str, style: &DiagramStyle) -> Result<(String, f32, f32), String> {
    let diagram = parse_mermaid(source)?;

    match diagram {
        MermaidDiagram::Flowchart(fc) => {
            let svg = super::flowchart::render_flowchart(&fc, style)?;
            Ok(svg)
        }
        MermaidDiagram::Sequence(seq) => {
            let svg = render_sequence(&seq, style)?;
            Ok(svg)
        }
        MermaidDiagram::ClassDiagram(cls) => {
            let svg = render_class(&cls, style)?;
            Ok(svg)
        }
        MermaidDiagram::StateDiagram(st) => {
            let svg = render_state(&st, style)?;
            Ok(svg)
        }
        MermaidDiagram::ErDiagram(er) => {
            let svg = render_er(&er, style)?;
            Ok(svg)
        }
    }
}

/// Escape XML special characters
pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ============================================
// FLOWCHART RENDERING (moved to flowchart.rs)
// ============================================

// Note: render_flowchart is in flowchart.rs

// ============================================
// SEQUENCE DIAGRAM RENDERING
// ============================================

fn render_sequence(
    diagram: &SequenceDiagram,
    style: &DiagramStyle,
) -> Result<(String, f32, f32), String> {
    if diagram.participants.is_empty() {
        return Ok(("<g></g>".to_string(), 100.0, 50.0));
    }

    let layout = LayoutEngine::new();
    let (positions, layout_elements, bbox) = layout.layout_sequence(diagram);

    let mut svg = String::new();
    let padding = 20.0;

    let mut participant_centers: HashMap<&str, f32> = HashMap::new();
    for participant in &diagram.participants {
        if let Some(pos) = positions.get(&participant.id) {
            let (cx, _) = pos.center();
            participant_centers.insert(participant.id.as_str(), cx);
        }
    }

    let left_edge = participant_centers
        .values()
        .copied()
        .fold(f32::MAX, f32::min)
        .min(40.0);
    let right_edge = participant_centers
        .values()
        .copied()
        .fold(f32::MIN, f32::max)
        .max(120.0);

    for participant in &diagram.participants {
        if let Some(pos) = positions.get(&participant.id) {
            let display_name = participant.alias.as_ref().unwrap_or(&participant.id);
            let label = escape_xml(display_name);
            let text_x = participant_centers
                .get(participant.id.as_str())
                .copied()
                .unwrap_or(pos.x + pos.width / 2.0);

            svg.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{}" stroke-width="1" rx="4" />"#,
                pos.x, pos.y, pos.width, pos.height, style.node_fill, style.node_stroke
            ));
            svg.push_str(&format!(
                r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
                text_x,
                pos.y + pos.height / 2.0 + style.font_size / 3.0,
                style.font_family,
                style.font_size,
                style.node_text,
                label
            ));
        }
    }

    let participant_bottom = diagram
        .participants
        .iter()
        .filter_map(|participant| positions.get(&participant.id))
        .map(|pos| pos.y + pos.height)
        .fold(bbox.y + 40.0, f32::max);
    let lifeline_start_y = participant_bottom + 8.0;
    let lifeline_end_y = bbox.bottom() - 20.0;
    for participant in &diagram.participants {
        if let Some(x) = participant_centers.get(participant.id.as_str()) {
            svg.push_str(&format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" stroke-dasharray="4,4" />"#,
                x, lifeline_start_y, x, lifeline_end_y, style.edge_stroke
            ));
        }
    }

    let mut message_y = lifeline_start_y + 24.0;
    for el in &layout_elements {
        match el {
            super::layout::SequenceLayoutElement::Message {
                from_x,
                to_x,
                y,
                label,
            } => {
                let label_span = (label.chars().count() as f32 * 3.5).max(8.0);
                let _max_edge = from_x.max(*to_x) + label_span;
                if *y > message_y {
                    message_y = *y;
                }
            }
            super::layout::SequenceLayoutElement::Activation { x, y, height } => {
                let _ = x;
                let bottom = y + height;
                if bottom > message_y {
                    message_y = bottom;
                }
            }
        }
    }

    let mut activation_starts: HashMap<String, Vec<f32>> = HashMap::new();
    svg.push_str(&render_sequence_elements(
        &diagram.elements,
        &participant_centers,
        style,
        &mut message_y,
        0,
        left_edge,
        right_edge,
        &mut activation_starts,
    ));

    Ok((
        svg,
        bbox.right() + padding,
        bbox.bottom().max(message_y + 20.0) + padding,
    ))
}

fn render_sequence_elements(
    elements: &[SequenceElement],
    participant_centers: &HashMap<&str, f32>,
    style: &DiagramStyle,
    message_y: &mut f32,
    block_depth: usize,
    left_edge: f32,
    right_edge: f32,
    activation_starts: &mut HashMap<String, Vec<f32>>,
) -> String {
    let mut svg = String::new();
    for element in elements {
        match element {
            SequenceElement::Message(msg) => {
                if let (Some(x1), Some(x2)) = (
                    participant_centers.get(msg.from.as_str()),
                    participant_centers.get(msg.to.as_str()),
                ) {
                    let is_right = x2 > x1;
                    let dash =
                        if msg.msg_type == MessageType::Dotted || msg.kind == MessageKind::Reply {
                            " stroke-dasharray=\"4,4\""
                        } else {
                            ""
                        };

                    svg.push_str(&format!(
                        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5"{} />"#,
                        x1, *message_y, x2, *message_y, style.edge_stroke, dash
                    ));

                    let arrow_dir = if is_right { -1.0 } else { 1.0 };
                    let arrow_x = *x2;
                    match msg.kind {
                        MessageKind::Async => {
                            let p1 = (arrow_x + arrow_dir * 10.0, *message_y - 5.0);
                            let p2 = (arrow_x + arrow_dir * 10.0, *message_y + 5.0);
                            svg.push_str(&format!(
                                r#"<polyline points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="none" stroke="{}" stroke-width="1.5" />"#,
                                p1.0, p1.1, arrow_x, *message_y, p2.0, p2.1, style.edge_stroke
                            ));
                        }
                        MessageKind::Sync => {
                            svg.push_str(&format!(
                                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" />"#,
                                arrow_x,
                                *message_y,
                                arrow_x + arrow_dir * 10.0,
                                *message_y - 5.0,
                                arrow_x + arrow_dir * 10.0,
                                *message_y + 5.0,
                                style.edge_stroke
                            ));
                        }
                        MessageKind::Reply => {
                            let p1 = (arrow_x + arrow_dir * 10.0, *message_y - 5.0);
                            let p2 = (arrow_x + arrow_dir * 10.0, *message_y + 5.0);
                            svg.push_str(&format!(
                                r#"<polyline points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="none" stroke="{}" stroke-width="1.5" />"#,
                                p1.0, p1.1, arrow_x, *message_y, p2.0, p2.1, style.edge_stroke
                            ));
                        }
                    }

                    if !msg.label.is_empty() {
                        svg.push_str(&format!(
                            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
                            (x1 + x2) / 2.0,
                            *message_y - 8.0,
                            style.font_family,
                            style.font_size * 0.85,
                            style.edge_text,
                            escape_xml(&msg.label)
                        ));
                    }
                }
                *message_y += 50.0;
            }
            SequenceElement::Activation(activation) => {
                if let Some(cx) = participant_centers.get(activation.participant.as_str()) {
                    activation_starts
                        .entry(activation.participant.clone())
                        .or_default()
                        .push(*message_y - 10.0);
                    svg.push_str(&format!(
                        r#"<rect x="{:.2}" y="{:.2}" width="8" height="16" fill="{}" stroke="{}" stroke-width="1" />"#,
                        cx - 4.0,
                        *message_y - 10.0,
                        style.node_fill,
                        style.node_stroke
                    ));
                }
                *message_y += 24.0;
            }
            SequenceElement::Deactivation(activation) => {
                if let Some(cx) = participant_centers.get(activation.participant.as_str()) {
                    if let Some(start) = activation_starts
                        .entry(activation.participant.clone())
                        .or_default()
                        .pop()
                    {
                        svg.push_str(&format!(
                            r#"<rect x="{:.2}" y="{:.2}" width="8" height="{:.2}" fill="{}" fill-opacity="0.35" stroke="{}" stroke-width="1" />"#,
                            cx - 4.0,
                            start,
                            (*message_y - start).max(16.0),
                            style.node_fill,
                            style.node_stroke
                        ));
                    }
                }
                *message_y += 24.0;
            }
            SequenceElement::Note {
                participant,
                position,
                text,
            } => {
                if let Some(cx) = participant_centers.get(participant.as_str()) {
                    let note_width = (text.chars().count() as f32 * 6.0 + 20.0)
                        .min(220.0)
                        .max(80.0);
                    let x = match position.as_str() {
                        "left" => cx - note_width - 12.0,
                        "right" => cx + 12.0,
                        _ => cx - note_width / 2.0,
                    };
                    let y = *message_y - 18.0;
                    svg.push_str(&format!(
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="28" rx="3" fill="{}" fill-opacity="0.25" stroke="{}" stroke-width="1" />"#,
                        x,
                        y,
                        note_width,
                        style.node_fill,
                        style.node_stroke
                    ));
                    svg.push_str(&format!(
                        r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}">{}</text>"#,
                        x + 8.0,
                        y + 18.0,
                        style.font_family,
                        style.font_size * 0.8,
                        style.node_text,
                        escape_xml(text)
                    ));
                }
                *message_y += 42.0;
            }
            SequenceElement::Block(block) => {
                let start_y = *message_y - 14.0;
                let inset = block_depth as f32 * 8.0;
                let block_left = left_edge - 36.0 + inset;
                let block_right = right_edge + 36.0 - inset;
                let block_kind = match block.block_type {
                    SequenceBlockType::Alt => "alt",
                    SequenceBlockType::Opt => "opt",
                    SequenceBlockType::Loop => "loop",
                    SequenceBlockType::Par => "par",
                    SequenceBlockType::Critical => "critical",
                };
                let title = if block.label.is_empty() {
                    block_kind.to_string()
                } else {
                    format!("{} {}", block_kind, block.label)
                };

                svg.push_str(&format!(
                    r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" font-weight="bold">{}</text>"#,
                    block_left + 6.0,
                    *message_y,
                    style.font_family,
                    style.font_size * 0.8,
                    style.edge_text,
                    escape_xml(&title)
                ));
                *message_y += 18.0;

                svg.push_str(&render_sequence_elements(
                    &block.messages,
                    participant_centers,
                    style,
                    message_y,
                    block_depth + 1,
                    left_edge,
                    right_edge,
                    activation_starts,
                ));

                for (label, branch_elements) in &block.else_branches {
                    let separator_y = *message_y + 2.0;
                    svg.push_str(&format!(
                        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" stroke-dasharray="5,3" />"#,
                        block_left,
                        separator_y,
                        block_right,
                        separator_y,
                        style.edge_stroke
                    ));
                    if !label.is_empty() {
                        svg.push_str(&format!(
                            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}">{}</text>"#,
                            block_left + 6.0,
                            separator_y - 4.0,
                            style.font_family,
                            style.font_size * 0.78,
                            style.edge_text,
                            escape_xml(label)
                        ));
                    }
                    *message_y = separator_y + 30.0;
                    svg.push_str(&render_sequence_elements(
                        branch_elements,
                        participant_centers,
                        style,
                        message_y,
                        block_depth + 1,
                        left_edge,
                        right_edge,
                        activation_starts,
                    ));
                }

                svg.push_str(&format!(
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="none" stroke="{}" stroke-width="1" stroke-dasharray="5,3" />"#,
                    block_left,
                    start_y,
                    (block_right - block_left).max(24.0),
                    (*message_y - start_y + 8.0).max(28.0),
                    style.edge_stroke
                ));
                *message_y += 10.0;
            }
        }
    }
    svg
}

// ============================================
// CLASS DIAGRAM RENDERING
// ============================================

fn render_class(
    diagram: &ClassDiagram,
    style: &DiagramStyle,
) -> Result<(String, f32, f32), String> {
    if diagram.classes.is_empty() {
        return Ok(("<g></g>".to_string(), 100.0, 50.0));
    }

    let layout = LayoutEngine::new();
    let (positions, bbox) = layout.layout_class(diagram);

    let mut svg = String::new();
    let padding = 20.0;

    // Draw classes
    for class in &diagram.classes {
        if let Some(pos) = positions.get(&class.name) {
            svg.push_str(&render_class_box(class, pos, style));
        }
    }

    // Draw relations
    for relation in &diagram.relations {
        let from_pos = positions.get(&relation.from);
        let to_pos = positions.get(&relation.to);

        if let (Some(from), Some(to)) = (from_pos, to_pos) {
            svg.push_str(&render_class_relation(relation, from, to, style));
        }
    }

    let total_width = bbox.right() + padding;
    let total_height = bbox.bottom() + padding;

    Ok((svg, total_width, total_height))
}

fn render_class_box(class: &ClassDefinition, pos: &LayoutPos, style: &DiagramStyle) -> String {
    let mut svg = String::new();

    // Main box
    svg.push_str(&format!(
        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
        pos.x, pos.y, pos.width, pos.height,
        style.node_fill, style.node_stroke
    ));

    let mut y = pos.y + style.font_size + 8.0;

    // Class name
    let effective_stereotype = if class.is_interface {
        class
            .stereotype
            .clone()
            .or_else(|| Some("interface".to_string()))
    } else {
        class.stereotype.clone()
    };
    let name_text = if let Some(stereo) = effective_stereotype {
        format!(
            "&lt;&lt;{}&gt;&gt; {}",
            escape_xml(&stereo),
            escape_xml(&class.name)
        )
    } else {
        escape_xml(&class.name)
    };
    let name_style = if class.is_abstract || class.is_interface {
        " font-style=\"italic\""
    } else {
        ""
    };

    svg.push_str(&format!(
        r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle" font-weight="bold"{}>{}</text>"#,
        pos.x + pos.width / 2.0,
        y,
        style.font_family,
        style.font_size,
        style.node_text,
        name_style,
        name_text
    ));

    // Divider line after name
    y += 6.0;
    svg.push_str(&format!(
        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
        pos.x,
        y,
        pos.x + pos.width,
        y,
        style.node_stroke
    ));

    // Attributes
    y += style.font_size + 4.0;
    for attr in &class.attributes {
        let vis = match attr.member.visibility {
            Visibility::Public => "+",
            Visibility::Private => "-",
            Visibility::Protected => "#",
            Visibility::Package => "~",
        };
        let attr_text = if let Some(ref t) = attr.type_annotation {
            format!("{} {}: {}", vis, attr.member.name, t)
        } else {
            format!("{} {}", vis, attr.member.name)
        };

        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="monospace" font-size="{:.1}" fill="{}"{}{}>{}</text>"#,
            pos.x + 8.0,
            y,
            style.font_size * 0.85,
            style.node_text,
            if attr.member.is_static {
                " text-decoration=\"underline\""
            } else {
                ""
            },
            if attr.member.is_abstract {
                " font-style=\"italic\""
            } else {
                ""
            },
            escape_xml(&attr_text)
        ));
        y += style.font_size * 0.9;
    }

    // Divider line before methods
    if !class.methods.is_empty() {
        y += 2.0;
        svg.push_str(&format!(
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
            pos.x,
            y,
            pos.x + pos.width,
            y,
            style.node_stroke
        ));
        y += style.font_size + 2.0;
    }

    // Methods
    for method in &class.methods {
        let vis = match method.member.visibility {
            Visibility::Public => "+",
            Visibility::Private => "-",
            Visibility::Protected => "#",
            Visibility::Package => "~",
        };

        let params: Vec<String> = method
            .parameters
            .iter()
            .map(|(name, t)| {
                if let Some(ty) = t {
                    format!("{}: {}", name, ty)
                } else {
                    name.clone()
                }
            })
            .collect();

        let method_text = if let Some(ref ret) = method.return_type {
            format!(
                "{} {}({}): {}",
                vis,
                method.member.name,
                params.join(", "),
                ret
            )
        } else {
            format!("{} {}({})", vis, method.member.name, params.join(", "))
        };

        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="monospace" font-size="{:.1}" fill="{}"{}{}>{}</text>"#,
            pos.x + 8.0,
            y,
            style.font_size * 0.85,
            style.node_text,
            if method.member.is_static {
                " text-decoration=\"underline\""
            } else {
                ""
            },
            if method.member.is_abstract {
                " font-style=\"italic\""
            } else {
                ""
            },
            escape_xml(&method_text)
        ));
        y += style.font_size * 0.9;
    }

    svg
}

fn render_class_relation(
    relation: &ClassRelation,
    from: &LayoutPos,
    to: &LayoutPos,
    style: &DiagramStyle,
) -> String {
    let mut svg = String::new();

    let (from_cx, from_cy) = from.center();
    let (to_cx, to_cy) = to.center();
    let angle = (to_cy - from_cy).atan2(to_cx - from_cx);

    let (x1, y1) = rect_boundary_point(from, angle);
    let (x2, y2) = rect_boundary_point(to, angle + std::f32::consts::PI);

    let line_style = match relation.relation_type {
        ClassRelationType::Dependency | ClassRelationType::Realization => {
            " stroke-dasharray=\"6,3\""
        }
        _ => "",
    };

    svg.push_str(&format!(
        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5"{} />"#,
        x1, y1, x2, y2, style.edge_stroke, line_style
    ));

    let (from_marker, to_marker) = match relation.relation_type {
        ClassRelationType::Inheritance => (Some("hollow_triangle"), None),
        ClassRelationType::Composition => (Some("filled_diamond"), None),
        ClassRelationType::Aggregation => (Some("hollow_diamond"), None),
        ClassRelationType::Association => (None, Some("arrow")),
        ClassRelationType::Dependency => (Some("hollow_triangle"), None),
        ClassRelationType::Realization => (Some("hollow_triangle"), None),
    };

    if let Some(marker) = to_marker {
        let angle = (y2 - y1).atan2(x2 - x1);
        svg.push_str(&draw_marker(marker, x2, y2, angle, style));
    }

    if let Some(marker) = from_marker {
        let angle = (y1 - y2).atan2(x1 - x2);
        svg.push_str(&draw_marker(marker, x1, y1, angle, style));
    }

    let angle = (y2 - y1).atan2(x2 - x1);
    let unit_x = angle.cos();
    let unit_y = angle.sin();
    let normal_x = -unit_y;
    let normal_y = unit_x;

    if let Some(label) = &relation.label {
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;
        let label_w = label.chars().count() as f32 * 7.0 + 10.0;
        let label_h = style.font_size * 0.95;
        svg.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" fill="{}" />"#,
            mx - label_w / 2.0,
            my - label_h + normal_y * 10.0,
            label_w,
            label_h + 4.0,
            style.background
        ));
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            mx + normal_x * 10.0,
            my + normal_y * 10.0,
            style.font_family,
            style.font_size * 0.8,
            style.edge_text,
            escape_xml(label)
        ));
    }

    if let Some(m) = &relation.multiplicity_from {
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}">{}</text>"#,
            x1 + unit_x * 12.0 + normal_x * 8.0,
            y1 + unit_y * 12.0 + normal_y * 8.0,
            style.font_family,
            style.font_size * 0.75,
            style.edge_text,
            escape_xml(m)
        ));
    }

    if let Some(m) = &relation.multiplicity_to {
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="end">{}</text>"#,
            x2 - unit_x * 12.0 + normal_x * 8.0,
            y2 - unit_y * 12.0 + normal_y * 8.0,
            style.font_family,
            style.font_size * 0.75,
            style.edge_text,
            escape_xml(m)
        ));
    }

    svg
}

fn draw_marker(marker_type: &str, x: f32, y: f32, angle: f32, style: &DiagramStyle) -> String {
    let cos = angle.cos();
    let sin = angle.sin();

    match marker_type {
        "arrow" => {
            let p1 = (x - cos * 12.0 + sin * 5.0, y - sin * 12.0 - cos * 5.0);
            let p2 = (x - cos * 12.0 - sin * 5.0, y - sin * 12.0 + cos * 5.0);
            format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" />"#,
                x, y, p1.0, p1.1, p2.0, p2.1, style.edge_stroke
            )
        }
        "hollow_triangle" => {
            let p1 = (x - cos * 14.0 + sin * 7.0, y - sin * 14.0 - cos * 7.0);
            let p2 = (x - cos * 14.0 - sin * 7.0, y - sin * 14.0 + cos * 7.0);
            let base = (x - cos * 10.0, y - sin * 10.0);
            format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                x, y, p1.0, p1.1, p2.0, p2.1, base.0, base.1, style.node_fill, style.edge_stroke
            )
        }
        "filled_diamond" => {
            let p1 = (x - cos * 16.0 + sin * 6.0, y - sin * 16.0 - cos * 6.0);
            let p2 = (x - cos * 16.0 - sin * 6.0, y - sin * 16.0 + cos * 6.0);
            let back = (x - cos * 24.0, y - sin * 24.0);
            format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" />"#,
                x, y, p1.0, p1.1, back.0, back.1, p2.0, p2.1, style.edge_stroke
            )
        }
        "hollow_diamond" => {
            let p1 = (x - cos * 16.0 + sin * 6.0, y - sin * 16.0 - cos * 6.0);
            let p2 = (x - cos * 16.0 - sin * 6.0, y - sin * 16.0 + cos * 6.0);
            let back = (x - cos * 24.0, y - sin * 24.0);
            format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="1" />"#,
                x, y, p1.0, p1.1, back.0, back.1, p2.0, p2.1, style.node_fill, style.edge_stroke
            )
        }
        _ => String::new(),
    }
}

// ============================================
// STATE DIAGRAM RENDERING
// ============================================

fn render_state(
    diagram: &StateDiagram,
    style: &DiagramStyle,
) -> Result<(String, f32, f32), String> {
    if diagram.states.is_empty() {
        return Ok(("<g></g>".to_string(), 100.0, 50.0));
    }

    let layout = LayoutEngine::new();
    let (positions, bbox) = layout.layout_state(diagram);

    let mut svg = String::new();
    let padding = 20.0;

    let mut child_state_ids: HashSet<&str> = HashSet::new();
    for state in &diagram.states {
        for child in &state.children {
            if let StateElement::State(child_state) = child {
                child_state_ids.insert(child_state.id.as_str());
            }
        }
    }

    // Draw transitions first (behind states)
    for transition in &diagram.transitions {
        if child_state_ids.contains(transition.from.as_str())
            || child_state_ids.contains(transition.to.as_str())
        {
            continue;
        }

        let from_pos = positions.get(&transition.from);
        let to_pos = positions.get(&transition.to);

        if let (Some(from), Some(to)) = (from_pos, to_pos) {
            svg.push_str(&render_state_transition(transition, from, to, style));
        }
    }

    // Draw states
    for state in &diagram.states {
        if child_state_ids.contains(state.id.as_str()) {
            continue;
        }

        if let Some(pos) = positions.get(&state.id) {
            svg.push_str(&render_state_node(
                state,
                &state.children,
                pos,
                style,
                &positions,
            ));
        }
    }

    let total_width = bbox.right() + padding;
    let total_height = bbox.bottom() + padding;

    Ok((svg, total_width, total_height))
}

fn render_state_node(
    state: &State,
    children: &[StateElement],
    pos: &LayoutPos,
    style: &DiagramStyle,
    positions: &HashMap<String, LayoutPos>,
) -> String {
    let mut svg = String::new();

    if state.is_start {
        // Start state (filled circle)
        svg.push_str(&format!(
            r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" />"#,
            pos.x + pos.width / 2.0,
            pos.y + pos.height / 2.0,
            pos.width / 2.0,
            style.node_stroke
        ));
    } else if state.is_end {
        // End state (circle with ring)
        let cx = pos.x + pos.width / 2.0;
        let cy = pos.y + pos.height / 2.0;
        svg.push_str(&format!(
            r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="2" />"#,
            cx,
            cy,
            pos.width / 2.0 - 3.0,
            style.node_stroke,
            style.node_stroke
        ));
        svg.push_str(&format!(
            r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="none" stroke="{}" stroke-width="2" />"#,
            cx, cy, pos.width / 2.0, style.node_stroke
        ));
    } else {
        svg.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
            pos.x, pos.y, pos.width, pos.height,
            10.0, style.node_fill, style.node_stroke
        ));

        let text_x = pos.x + pos.width / 2.0;
        let text_y = if state.is_composite {
            pos.y + style.font_size + 8.0
        } else {
            pos.y + pos.height / 2.0 + style.font_size / 3.0
        };
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            text_x, text_y, style.font_family, style.font_size, style.node_text, escape_xml(&state.label)
        ));

        if state.is_composite {
            svg.push_str(&render_composite_state_contents(
                state, children, pos, style, positions,
            ));
        } else {
            for child in children {
                if let StateElement::Note {
                    state: note_state,
                    text,
                } = child
                {
                    if note_state == &state.id && !text.is_empty() {
                        svg.push_str(&render_state_note(note_state, text, pos, style));
                    }
                }
            }
        }
    }

    svg
}

fn render_composite_state_contents(
    state: &State,
    children: &[StateElement],
    parent_pos: &LayoutPos,
    style: &DiagramStyle,
    positions: &HashMap<String, LayoutPos>,
) -> String {
    let mut svg = String::new();
    let mut child_states: Vec<&State> = Vec::new();
    let mut child_transitions: Vec<&StateTransition> = Vec::new();
    let mut child_notes: Vec<(&str, &str)> = Vec::new();

    for child in children {
        match child {
            StateElement::State(child_state) => child_states.push(child_state),
            StateElement::Transition(transition) => child_transitions.push(transition),
            StateElement::Note { state, text } if !text.is_empty() => {
                child_notes.push((state.as_str(), text.as_str()))
            }
            StateElement::Note { .. } => {}
        }
    }

    if child_states.is_empty() {
        return svg;
    }

    let mut child_positions: HashMap<String, LayoutPos> = HashMap::new();
    let inner_top = parent_pos.y + 60.0;
    let mut child_y = inner_top;
    let child_x = parent_pos.x + 20.0;
    let child_width = (parent_pos.width - 40.0).max(80.0);
    let child_height = 40.0;
    let child_spacing = 14.0;

    for child_state in child_states {
        if child_state.id == state.id {
            continue;
        }
        child_positions.insert(
            child_state.id.clone(),
            LayoutPos::new(child_x, child_y, child_width, child_height),
        );
        svg.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="10.00" fill="{}" stroke="{}" stroke-width="1.5" />"#,
            child_x, child_y, child_width, child_height, style.node_fill, style.node_stroke
        ));
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            child_x + child_width / 2.0,
            child_y + child_height / 2.0 + style.font_size / 3.0,
            style.font_family,
            style.font_size,
            style.node_text,
            escape_xml(&child_state.label)
        ));

        child_y += child_height + child_spacing;
    }

    for transition in child_transitions {
        let from = child_positions
            .get(&transition.from)
            .or_else(|| positions.get(&transition.from));
        let to = child_positions
            .get(&transition.to)
            .or_else(|| positions.get(&transition.to));
        if let (Some(from_pos), Some(to_pos)) = (from, to) {
            svg.push_str(&render_state_transition(
                transition, from_pos, to_pos, style,
            ));
        }
    }

    for (note_state, text) in child_notes {
        if let Some(target) = child_positions
            .get(note_state)
            .or_else(|| positions.get(note_state))
        {
            svg.push_str(&render_state_note(note_state, text, target, style));
        }
    }

    svg
}

fn render_state_note(
    _state_id: &str,
    text: &str,
    state_pos: &LayoutPos,
    style: &DiagramStyle,
) -> String {
    let note_width = (text.chars().count() as f32 * 6.0).clamp(72.0, 180.0);
    let note_height = 26.0;
    let x = state_pos.x + state_pos.width + 28.0;
    let y = state_pos.y + 4.0;

    format!(
        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="3" fill="{}" fill-opacity="0.25" stroke="{}" stroke-width="1" /><text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}">{}</text>"#,
        x,
        y,
        note_width,
        note_height,
        style.node_fill,
        style.node_stroke,
        x + 8.0,
        y + note_height * 0.65,
        style.font_family,
        style.font_size * 0.8,
        style.edge_text,
        escape_xml(text)
    )
}

fn render_state_transition(
    transition: &StateTransition,
    from: &LayoutPos,
    to: &LayoutPos,
    style: &DiagramStyle,
) -> String {
    let mut svg = String::new();

    let (from_cx, from_cy) = from.center();
    let (to_cx, to_cy) = to.center();
    let center_angle = (to_cy - from_cy).atan2(to_cx - from_cx);

    let (px1, py1) = if from.width == from.height && from.width < 30.0 {
        (
            from_cx + center_angle.cos() * (from.width / 2.0),
            from_cy + center_angle.sin() * (from.width / 2.0),
        )
    } else {
        rect_boundary_point(from, center_angle)
    };

    let (px2, py2) = if to.width == to.height && to.width < 30.0 {
        (
            to_cx + (center_angle + std::f32::consts::PI).cos() * (to.width / 2.0 + 5.0),
            to_cy + (center_angle + std::f32::consts::PI).sin() * (to.width / 2.0 + 5.0),
        )
    } else {
        let (x, y) = rect_boundary_point(to, center_angle + std::f32::consts::PI);
        (x + center_angle.cos() * 5.0, y + center_angle.sin() * 5.0)
    };

    // Line
    svg.push_str(&format!(
        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5" />"#,
        px1, py1, px2, py2, style.edge_stroke
    ));

    // Arrow
    let arrow_angle = (py1 - py2).atan2(px1 - px2);
    let ax = px2;
    let ay = py2;
    let p1 = (
        ax + arrow_angle.cos() * 10.0 - arrow_angle.sin() * 5.0,
        ay + arrow_angle.sin() * 10.0 + arrow_angle.cos() * 5.0,
    );
    let p2 = (
        ax + arrow_angle.cos() * 10.0 + arrow_angle.sin() * 5.0,
        ay + arrow_angle.sin() * 10.0 - arrow_angle.cos() * 5.0,
    );

    svg.push_str(&format!(
        r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" />"#,
        ax, ay, p1.0, p1.1, p2.0, p2.1, style.edge_stroke
    ));

    // Label
    if let Some(ref label) = transition.label {
        let mut label_x = (px1 + px2) / 2.0;
        let is_upward = py2 < py1;
        let label_y = if is_upward {
            (py1 + py2) / 2.0 - 10.0
        } else {
            (py1 + py2) / 2.0 + 14.0
        };

        if (px2 - px1).abs() < 20.0 {
            label_x += if is_upward { -24.0 } else { 24.0 };
        }
        let label_width = label.chars().count() as f32 * 6.8 + 8.0;
        let label_height = style.font_size * 0.8 + 6.0;

        svg.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" fill="{}" />"#,
            label_x - label_width / 2.0,
            label_y - label_height + 2.0,
            label_width,
            label_height,
            style.background
        ));
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            label_x,
            label_y,
            style.font_family,
            style.font_size * 0.85,
            style.edge_text,
            escape_xml(label)
        ));
    }

    svg
}

// ============================================
// ER DIAGRAM RENDERING
// ============================================

fn render_er(diagram: &ErDiagram, style: &DiagramStyle) -> Result<(String, f32, f32), String> {
    if diagram.entities.is_empty() {
        return Ok(("<g></g>".to_string(), 100.0, 50.0));
    }

    let layout = LayoutEngine::new();
    let (positions, bbox) = layout.layout_er(diagram);

    let mut svg = String::new();
    let padding = 20.0;

    // Draw relationships first
    for relation in &diagram.relationships {
        let from_pos = positions.get(&relation.from);
        let to_pos = positions.get(&relation.to);

        if let (Some(from), Some(to)) = (from_pos, to_pos) {
            svg.push_str(&render_er_relationship(relation, from, to, style));
        }
    }

    // Draw entities
    for entity in &diagram.entities {
        if let Some(pos) = positions.get(&entity.name) {
            svg.push_str(&render_er_entity(entity, pos, style));
        }
    }

    let total_width = bbox.right() + padding;
    let total_height = bbox.bottom() + padding;

    Ok((svg, total_width, total_height))
}

fn render_er_entity(entity: &ErEntity, pos: &LayoutPos, style: &DiagramStyle) -> String {
    let mut svg = String::new();

    // Entity box
    svg.push_str(&format!(
        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
        pos.x, pos.y, pos.width, pos.height,
        style.node_fill, style.node_stroke
    ));

    let mut y = pos.y + style.font_size + 6.0;

    // Entity name (bold)
    svg.push_str(&format!(
        r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle" font-weight="bold">{}</text>"#,
        pos.x + pos.width / 2.0, y, style.font_family, style.font_size, style.node_text, escape_xml(&entity.name)
    ));

    // Attributes
    y += style.font_size + 4.0;
    for attr in &entity.attributes {
        let marker = if attr.is_key { "*" } else { "" };
        let attr_name = if attr.is_composite {
            format!("[{}]", attr.name)
        } else {
            attr.name.clone()
        };
        let attr_text = format!("{}{}", marker, attr_name);

        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}">{}</text>"#,
            pos.x + 8.0,
            y,
            style.font_family,
            style.font_size * 0.9,
            style.node_text,
            escape_xml(&attr_text)
        ));
        y += style.font_size;
    }

    svg
}

fn render_er_relationship(
    relation: &ErRelationship,
    from: &LayoutPos,
    to: &LayoutPos,
    style: &DiagramStyle,
) -> String {
    let mut svg = String::new();

    let (from_cx, from_cy) = from.center();
    let (to_cx, to_cy) = to.center();
    let angle = (to_cy - from_cy).atan2(to_cx - from_cx);
    let (x1, y1) = rect_boundary_point(from, angle);
    let (x2, y2) = rect_boundary_point(to, angle + std::f32::consts::PI);

    // Line
    svg.push_str(&format!(
        r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5" />"#,
        x1, y1, x2, y2, style.edge_stroke
    ));

    svg.push_str(&render_er_cardinality_marker(
        x1,
        y1,
        angle,
        &relation.from_cardinality,
        style,
    ));
    svg.push_str(&render_er_cardinality_marker(
        x2,
        y2,
        angle + std::f32::consts::PI,
        &relation.to_cardinality,
        style,
    ));

    if let Some(label) = &relation.label {
        svg.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.1}" fill="{}" text-anchor="middle">{}</text>"#,
            (x1 + x2) / 2.0,
            (y1 + y2) / 2.0 - 8.0,
            style.font_family,
            style.font_size * 0.8,
            style.edge_text,
            escape_xml(label)
        ));
    }

    svg
}

fn rect_boundary_point(rect: &LayoutPos, angle: f32) -> (f32, f32) {
    let (cx, cy) = rect.center();
    let dx = angle.cos();
    let dy = angle.sin();
    let half_w = rect.width / 2.0;
    let half_h = rect.height / 2.0;

    let tx = if dx.abs() > 1e-5 {
        half_w / dx.abs()
    } else {
        f32::INFINITY
    };
    let ty = if dy.abs() > 1e-5 {
        half_h / dy.abs()
    } else {
        f32::INFINITY
    };
    let t = tx.min(ty);

    (cx + dx * t, cy + dy * t)
}

fn render_er_cardinality_marker(
    x: f32,
    y: f32,
    angle: f32,
    cardinality: &ErCardinality,
    style: &DiagramStyle,
) -> String {
    let ux = angle.cos();
    let uy = angle.sin();
    let nx = -uy;
    let ny = ux;

    let mut marker = String::new();

    let draw_one = |dist: f32| {
        format!(
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5" />"#,
            x + ux * dist + nx * 6.0,
            y + uy * dist + ny * 6.0,
            x + ux * dist - nx * 6.0,
            y + uy * dist - ny * 6.0,
            style.edge_stroke
        )
    };

    let draw_zero = |dist: f32| {
        format!(
            r#"<circle cx="{:.2}" cy="{:.2}" r="4.50" fill="{}" stroke="{}" stroke-width="1.2" />"#,
            x + ux * dist,
            y + uy * dist,
            style.background,
            style.edge_stroke
        )
    };

    let draw_many = |dist: f32| {
        let cx = x + ux * dist;
        let cy = y + uy * dist;
        format!(
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5" /><line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5" /><line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5" />"#,
            cx,
            cy,
            cx + ux * 8.0 + nx * 8.0,
            cy + uy * 8.0 + ny * 8.0,
            style.edge_stroke,
            cx,
            cy,
            cx + ux * 10.0,
            cy + uy * 10.0,
            style.edge_stroke,
            cx,
            cy,
            cx + ux * 8.0 - nx * 8.0,
            cy + uy * 8.0 - ny * 8.0,
            style.edge_stroke
        )
    };

    match cardinality {
        ErCardinality::ExactlyOne => {
            marker.push_str(&draw_one(8.0));
            marker.push_str(&draw_one(14.0));
        }
        ErCardinality::ZeroOrOne => {
            marker.push_str(&draw_zero(8.0));
            marker.push_str(&draw_one(16.0));
        }
        ErCardinality::ZeroOrMore => {
            marker.push_str(&draw_zero(8.0));
            marker.push_str(&draw_many(16.0));
        }
        ErCardinality::OneOrMore => {
            marker.push_str(&draw_one(8.0));
            marker.push_str(&draw_many(16.0));
        }
    }

    marker
}

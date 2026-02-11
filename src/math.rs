use crate::fonts::TextMeasure;
use latex2mathml::{latex_to_mathml, DisplayStyle};
use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::Reader as XmlReader;

#[derive(Debug)]
enum MathNode {
    Row(Vec<MathNode>),
    Ident(String),
    Number(String),
    Operator(String),
    Text(String),
    Sup {
        base: Box<MathNode>,
        sup: Box<MathNode>,
    },
    Sub {
        base: Box<MathNode>,
        sub: Box<MathNode>,
    },
    SubSup {
        base: Box<MathNode>,
        sub: Box<MathNode>,
        sup: Box<MathNode>,
    },
    Frac {
        num: Box<MathNode>,
        den: Box<MathNode>,
    },
    Sqrt {
        radicand: Box<MathNode>,
    },
    UnderOver {
        base: Box<MathNode>,
        under: Option<Box<MathNode>>,
        over: Option<Box<MathNode>>,
    },
    Space(f32),
}

pub struct MathResult {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub svg_fragment: String,
}

pub fn render_math<T: TextMeasure>(
    latex: &str,
    font_size: f32,
    text_color: &str,
    measure: &mut T,
    display: bool,
) -> Result<MathResult, String> {
    render_math_at(latex, font_size, text_color, measure, display, 0.0, 0.0)
}

pub fn render_math_at<T: TextMeasure>(
    latex: &str,
    font_size: f32,
    text_color: &str,
    measure: &mut T,
    display: bool,
    x: f32,
    baseline_y: f32,
) -> Result<MathResult, String> {
    let style = if display {
        DisplayStyle::Block
    } else {
        DisplayStyle::Inline
    };

    let mathml = latex_to_mathml(latex, style)
        .map_err(|e| format!("LaTeX parse error: {:?}", e))?;

    let root = parse_mathml(&mathml)?;
    let mbox = layout_node(&root, font_size, text_color, measure, x, baseline_y);

    Ok(MathResult {
        width: mbox.width,
        ascent: mbox.ascent,
        descent: mbox.descent,
        svg_fragment: mbox.svg,
    })
}

struct MathBox {
    width: f32,
    ascent: f32,
    descent: f32,
    svg: String,
}

fn parse_mathml(mathml: &str) -> Result<MathNode, String> {
    let mut reader = XmlReader::from_str(mathml);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<(String, Vec<MathNode>)> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                stack.push((name, Vec::new()));
            }
            Ok(XmlEvent::Text(ref e)) => {
                let text = e.decode().unwrap_or_default().to_string();
                if !text.is_empty() {
                    if let Some((_, children)) = stack.last_mut() {
                        children.push(MathNode::Text(text));
                    }
                }
            }
            Ok(XmlEvent::End(_)) => {
                if let Some((tag, children)) = stack.pop() {
                    let node = build_node(&tag, children);
                    if let Some((_, parent_children)) = stack.last_mut() {
                        parent_children.push(node);
                    } else {
                        return Ok(node);
                    }
                }
            }
            Ok(XmlEvent::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "mspace" {
                    let mut width_em = 0.0;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"width" {
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if let Some(stripped) = val.strip_suffix("em") {
                                width_em = stripped.parse().unwrap_or(0.0);
                            }
                        }
                    }
                    let node = MathNode::Space(width_em);
                    if let Some((_, parent_children)) = stack.last_mut() {
                        parent_children.push(node);
                    }
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(MathNode::Row(Vec::new()))
}

fn build_node(tag: &str, mut children: Vec<MathNode>) -> MathNode {
    match tag {
        "mi" => {
            let text = extract_text(&children);
            MathNode::Ident(text)
        }
        "mn" => {
            let text = extract_text(&children);
            MathNode::Number(text)
        }
        "mo" => {
            let text = extract_text(&children);
            MathNode::Operator(text)
        }
        "mtext" => {
            let text = extract_text(&children);
            MathNode::Text(text)
        }
        "msup" if children.len() >= 2 => {
            let sup = children.pop().unwrap();
            let base = children.pop().unwrap();
            MathNode::Sup {
                base: Box::new(base),
                sup: Box::new(sup),
            }
        }
        "msub" if children.len() >= 2 => {
            let sub = children.pop().unwrap();
            let base = children.pop().unwrap();
            MathNode::Sub {
                base: Box::new(base),
                sub: Box::new(sub),
            }
        }
        "msubsup" if children.len() >= 3 => {
            let sup = children.pop().unwrap();
            let sub = children.pop().unwrap();
            let base = children.pop().unwrap();
            MathNode::SubSup {
                base: Box::new(base),
                sub: Box::new(sub),
                sup: Box::new(sup),
            }
        }
        "mfrac" if children.len() >= 2 => {
            let den = children.pop().unwrap();
            let num = children.pop().unwrap();
            MathNode::Frac {
                num: Box::new(num),
                den: Box::new(den),
            }
        }
        "msqrt" => {
            let radicand = if children.len() == 1 {
                children.pop().unwrap()
            } else {
                MathNode::Row(children)
            };
            MathNode::Sqrt {
                radicand: Box::new(radicand),
            }
        }
        "mover" if children.len() >= 2 => {
            let over = children.pop().unwrap();
            let base = children.pop().unwrap();
            MathNode::UnderOver {
                base: Box::new(base),
                under: None,
                over: Some(Box::new(over)),
            }
        }
        "munder" if children.len() >= 2 => {
            let under = children.pop().unwrap();
            let base = children.pop().unwrap();
            MathNode::UnderOver {
                base: Box::new(base),
                under: Some(Box::new(under)),
                over: None,
            }
        }
        "munderover" if children.len() >= 3 => {
            let over = children.pop().unwrap();
            let under = children.pop().unwrap();
            let base = children.pop().unwrap();
            MathNode::UnderOver {
                base: Box::new(base),
                under: Some(Box::new(under)),
                over: Some(Box::new(over)),
            }
        }
        "math" | "mrow" | "mstyle" | "mpadded" => {
            if children.len() == 1 {
                children.pop().unwrap()
            } else {
                MathNode::Row(children)
            }
        }
        _ => {
            if children.len() == 1 {
                children.pop().unwrap()
            } else {
                MathNode::Row(children)
            }
        }
    }
}

fn extract_text(children: &[MathNode]) -> String {
    let mut s = String::new();
    for child in children {
        match child {
            MathNode::Text(t)
            | MathNode::Ident(t)
            | MathNode::Number(t)
            | MathNode::Operator(t) => s.push_str(t),
            _ => {}
        }
    }
    s
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn measure_token<T: TextMeasure>(
    text: &str,
    font_size: f32,
    italic: bool,
    measure: &mut T,
) -> (f32, f32) {
    measure.measure_text(text, font_size, false, false, italic, None)
}

fn layout_node<T: TextMeasure>(
    node: &MathNode,
    font_size: f32,
    color: &str,
    measure: &mut T,
    x: f32,
    baseline_y: f32,
) -> MathBox {
    match node {
        MathNode::Ident(text) => {
            let italic =
                text.len() == 1 && text.chars().next().map_or(false, |c| c.is_alphabetic());
            let (w, _h) = measure_token(text, font_size, italic, measure);
            let style = if italic {
                " font-style=\"italic\""
            } else {
                ""
            };
            let svg = format!(
                r#"<text x="{:.2}" y="{:.2}" font-family="serif" font-size="{:.2}" fill="{}"{}>{}</text>"#,
                x, baseline_y, font_size, color, style, escape_xml(text)
            );
            MathBox {
                width: w,
                ascent: font_size * 0.75,
                descent: font_size * 0.25,
                svg,
            }
        }
        MathNode::Number(text) => {
            let (w, _h) = measure_token(text, font_size, false, measure);
            let svg = format!(
                r#"<text x="{:.2}" y="{:.2}" font-family="serif" font-size="{:.2}" fill="{}">{}</text>"#,
                x, baseline_y, font_size, color, escape_xml(text)
            );
            MathBox {
                width: w,
                ascent: font_size * 0.75,
                descent: font_size * 0.25,
                svg,
            }
        }
        MathNode::Operator(text) => {
            let is_large = is_large_operator(text);
            let effective_size = if is_large {
                font_size * 1.4
            } else {
                font_size
            };
            let (w, _h) = measure_token(text, effective_size, false, measure);
            let spacing = font_size * 0.15;
            let total_w = w + spacing * 2.0;
            let y_offset = if is_large {
                baseline_y + (effective_size - font_size) * 0.2
            } else {
                baseline_y
            };
            let svg = format!(
                r#"<text x="{:.2}" y="{:.2}" font-family="serif" font-size="{:.2}" fill="{}">{}</text>"#,
                x + spacing,
                y_offset,
                effective_size,
                color,
                escape_xml(text)
            );
            MathBox {
                width: total_w,
                ascent: if is_large {
                    effective_size * 0.8
                } else {
                    font_size * 0.75
                },
                descent: if is_large {
                    effective_size * 0.3
                } else {
                    font_size * 0.25
                },
                svg,
            }
        }
        MathNode::Text(text) => {
            let (w, _h) = measure_token(text, font_size, false, measure);
            let svg = format!(
                r#"<text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="{:.2}" fill="{}">{}</text>"#,
                x, baseline_y, font_size, color, escape_xml(text)
            );
            MathBox {
                width: w,
                ascent: font_size * 0.75,
                descent: font_size * 0.25,
                svg,
            }
        }
        MathNode::Space(em) => MathBox {
            width: font_size * em,
            ascent: 0.0,
            descent: 0.0,
            svg: String::new(),
        },
        MathNode::Row(children) => layout_row(children, font_size, color, measure, x, baseline_y),
        MathNode::Sup { base, sup } => {
            let base_box = layout_node(base, font_size, color, measure, x, baseline_y);

            let sup_size = font_size * 0.7;
            let sup_y = baseline_y - base_box.ascent * 0.55;
            let sup_box =
                layout_node(sup, sup_size, color, measure, x + base_box.width, sup_y);

            let total_width = base_box.width + sup_box.width;
            let ascent = base_box
                .ascent
                .max(sup_box.ascent + base_box.ascent * 0.55);
            let descent = base_box.descent;

            MathBox {
                width: total_width,
                ascent,
                descent,
                svg: format!("{}{}", base_box.svg, sup_box.svg),
            }
        }
        MathNode::Sub { base, sub } => {
            let base_box = layout_node(base, font_size, color, measure, x, baseline_y);

            let sub_size = font_size * 0.7;
            let sub_y = baseline_y + base_box.descent + sub_size * 0.35;
            let sub_box =
                layout_node(sub, sub_size, color, measure, x + base_box.width, sub_y);

            let total_width = base_box.width + sub_box.width;
            let ascent = base_box.ascent;
            let descent = (base_box.descent + sub_size * 0.35 + sub_box.descent)
                .max(base_box.descent);

            MathBox {
                width: total_width,
                ascent,
                descent,
                svg: format!("{}{}", base_box.svg, sub_box.svg),
            }
        }
        MathNode::SubSup { base, sub, sup } => {
            let base_box = layout_node(base, font_size, color, measure, x, baseline_y);

            let script_size = font_size * 0.7;

            let sup_y = baseline_y - base_box.ascent * 0.55;
            let sup_box = layout_node(
                sup,
                script_size,
                color,
                measure,
                x + base_box.width,
                sup_y,
            );

            let sub_y = baseline_y + base_box.descent + script_size * 0.35;
            let sub_box = layout_node(
                sub,
                script_size,
                color,
                measure,
                x + base_box.width,
                sub_y,
            );

            let script_width = sup_box.width.max(sub_box.width);
            let total_width = base_box.width + script_width;
            let ascent = base_box
                .ascent
                .max(sup_box.ascent + base_box.ascent * 0.55);
            let descent = (base_box.descent + script_size * 0.35 + sub_box.descent)
                .max(base_box.descent);

            MathBox {
                width: total_width,
                ascent,
                descent,
                svg: format!("{}{}{}", base_box.svg, sup_box.svg, sub_box.svg),
            }
        }
        MathNode::UnderOver {
            base,
            under,
            over,
        } => layout_underover(
            base,
            under.as_deref(),
            over.as_deref(),
            font_size,
            color,
            measure,
            x,
            baseline_y,
        ),
        MathNode::Frac { num, den } => {
            let frac_size = font_size * 0.85;

            let num_box = layout_node(num, frac_size, color, measure, 0.0, 0.0);
            let den_box = layout_node(den, frac_size, color, measure, 0.0, 0.0);

            let max_width = num_box.width.max(den_box.width);
            let padding = font_size * 0.2;
            let frac_width = max_width + padding * 2.0;

            let rule_y = baseline_y - font_size * 0.3;
            let gap = font_size * 0.15;

            let num_baseline = rule_y - gap - num_box.descent;
            let den_baseline = rule_y + gap + den_box.ascent;

            let num_x = x + (frac_width - num_box.width) / 2.0;
            let den_x = x + (frac_width - den_box.width) / 2.0;

            let num_rendered = layout_node(num, frac_size, color, measure, num_x, num_baseline);
            let den_rendered = layout_node(den, frac_size, color, measure, den_x, den_baseline);

            let rule_svg = format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
                x, rule_y, x + frac_width, rule_y, color
            );

            let ascent =
                (baseline_y - num_baseline + num_rendered.ascent).max(font_size * 0.75);
            let descent =
                (den_baseline - baseline_y + den_rendered.descent).max(font_size * 0.25);

            MathBox {
                width: frac_width,
                ascent,
                descent,
                svg: format!("{}{}{}", num_rendered.svg, rule_svg, den_rendered.svg),
            }
        }
        MathNode::Sqrt { radicand } => {
            let inner = layout_node(radicand, font_size, color, measure, 0.0, 0.0);

            let radical_width = font_size * 0.6;
            let padding = font_size * 0.1;
            let overbar_gap = font_size * 0.15;
            let total_width = radical_width + inner.width + padding;

            let inner_box = layout_node(
                radicand,
                font_size,
                color,
                measure,
                x + radical_width,
                baseline_y,
            );

            let top_y = baseline_y - inner_box.ascent - overbar_gap;
            let bottom_y = baseline_y + inner_box.descent;

            let radical_svg = format!(
                r#"<path d="M {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2}" stroke="{}" stroke-width="1.2" fill="none" />"#,
                x,
                baseline_y - font_size * 0.15,
                x + radical_width * 0.35,
                baseline_y,
                x + radical_width * 0.6,
                top_y,
                x + radical_width + inner_box.width + padding,
                top_y,
                color
            );

            let ascent = (baseline_y - top_y).max(inner_box.ascent + overbar_gap);
            let descent = inner_box.descent.max(bottom_y - baseline_y);

            MathBox {
                width: total_width,
                ascent,
                descent,
                svg: format!("{}{}", radical_svg, inner_box.svg),
            }
        }
    }
}

fn layout_row<T: TextMeasure>(
    children: &[MathNode],
    font_size: f32,
    color: &str,
    measure: &mut T,
    start_x: f32,
    baseline_y: f32,
) -> MathBox {
    let mut cx = start_x;
    let mut svg = String::new();
    let mut max_ascent: f32 = font_size * 0.75;
    let mut max_descent: f32 = font_size * 0.25;

    for child in children {
        let child_box = layout_node(child, font_size, color, measure, cx, baseline_y);
        max_ascent = max_ascent.max(child_box.ascent);
        max_descent = max_descent.max(child_box.descent);
        cx += child_box.width;
        svg.push_str(&child_box.svg);
    }

    MathBox {
        width: cx - start_x,
        ascent: max_ascent,
        descent: max_descent,
        svg,
    }
}

fn layout_underover<T: TextMeasure>(
    base: &MathNode,
    under: Option<&MathNode>,
    over: Option<&MathNode>,
    font_size: f32,
    color: &str,
    measure: &mut T,
    x: f32,
    baseline_y: f32,
) -> MathBox {
    let base_box = layout_node(base, font_size, color, measure, 0.0, 0.0);
    let script_size = font_size * 0.65;
    let gap = font_size * 0.15;

    let over_box = over.map(|o| layout_node(o, script_size, color, measure, 0.0, 0.0));
    let under_box = under.map(|u| layout_node(u, script_size, color, measure, 0.0, 0.0));

    let max_width = [
        base_box.width,
        over_box.as_ref().map_or(0.0, |b| b.width),
        under_box.as_ref().map_or(0.0, |b| b.width),
    ]
    .into_iter()
    .fold(0.0f32, f32::max);

    let mut svg = String::new();
    let mut total_ascent = base_box.ascent;
    let mut total_descent = base_box.descent;

    let base_x = x + (max_width - base_box.width) / 2.0;
    let base_rendered = layout_node(base, font_size, color, measure, base_x, baseline_y);
    svg.push_str(&base_rendered.svg);

    if let (Some(over_node), Some(ob)) = (over, &over_box) {
        let over_baseline = baseline_y - base_box.ascent - gap - ob.descent;
        let over_x = x + (max_width - ob.width) / 2.0;
        let over_rendered =
            layout_node(over_node, script_size, color, measure, over_x, over_baseline);
        svg.push_str(&over_rendered.svg);
        total_ascent = base_box.ascent + gap + ob.ascent + ob.descent;
    }

    if let (Some(under_node), Some(ub)) = (under, &under_box) {
        let under_baseline = baseline_y + base_box.descent + gap + ub.ascent;
        let under_x = x + (max_width - ub.width) / 2.0;
        let under_rendered =
            layout_node(under_node, script_size, color, measure, under_x, under_baseline);
        svg.push_str(&under_rendered.svg);
        total_descent = base_box.descent + gap + ub.ascent + ub.descent;
    }

    MathBox {
        width: max_width,
        ascent: total_ascent,
        descent: total_descent,
        svg,
    }
}

fn is_large_operator(text: &str) -> bool {
    matches!(
        text,
        "∑" | "∏"
            | "∐"
            | "⋀"
            | "⋁"
            | "⋂"
            | "⋃"
            | "∫"
            | "∬"
            | "∭"
            | "∮"
            | "⨁"
            | "⨂"
            | "⨀"
    )
}

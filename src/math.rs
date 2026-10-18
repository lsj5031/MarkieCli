use crate::fonts::TextMeasure;
use latex2mathml::{DisplayStyle, latex_to_mathml};
use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::Reader as XmlReader;
use std::collections::HashMap;

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
        line_thickness: Option<f32>, // None = default, Some(0.0) = no line
    },
    Sqrt {
        radicand: Box<MathNode>,
    },
    Root {
        radicand: Box<MathNode>,
        index: Box<MathNode>,
    },
    UnderOver {
        base: Box<MathNode>,
        under: Option<Box<MathNode>>,
        over: Option<Box<MathNode>>,
    },
    Space(f32),
    /// Table for matrices, cases, aligned equations
    Table {
        rows: Vec<Vec<MathNode>>,
        column_align: Vec<String>, // "left", "center", "right"
    },
    /// Stretchy operator (parentheses, brackets that scale)
    StretchyOp {
        op: String,
        form: String, // "prefix", "postfix", "infix"
    },
}

impl Default for MathNode {
    fn default() -> Self {
        MathNode::Row(Vec::new())
    }
}

#[derive(Debug)]
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

/// Map unsupported LaTeX environments to supported equivalents for latex2mathml.
///
/// These replacements are safe from false substring matches because `\begin{` and
/// `\end{` are LaTeX command sequences that won't appear as arbitrary substrings
/// in well-formed LaTeX input.
fn preprocess_latex(latex: &str) -> String {
    let mut result = String::with_capacity(latex.len());

    // aligned → align (supported by latex2mathml)
    // cases → \left\{ + matrix + \right. (preserves the semantic left curly brace)
    let latex = latex.replace("\\begin{aligned}", "\\begin{align}");
    let latex = latex.replace("\\end{aligned}", "\\end{align}");
    let latex = latex.replace("\\begin{cases}", "\\left\\{\\begin{matrix}");
    let latex = latex.replace("\\end{cases}", "\\end{matrix}\\right.");

    // Single-pass scan for \begin{array}{...} → \begin{matrix}
    let bytes = latex.as_bytes();
    let len = bytes.len();
    let begin_array = b"\\begin{array}";
    let pat_len = begin_array.len();
    let mut i = 0;

    while i < len {
        if i + pat_len <= len && &bytes[i..i + pat_len] == begin_array {
            result.push_str("\\begin{matrix}");
            i += pat_len;
            // Skip the optional column-alignment spec: {cc}, {l|r}, etc.
            if i < len && bytes[i] == b'{' {
                let mut depth = 1;
                i += 1;
                while i < len && depth > 0 {
                    if bytes[i] == b'{' {
                        depth += 1;
                    } else if bytes[i] == b'}' {
                        depth -= 1;
                    }
                    i += 1;
                }
            }
        } else {
            let ch = latex[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    result = result.replace("\\end{array}", "\\end{matrix}");

    result
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
    let root = latex_node(latex, display)?;
    let text_color = crate::xml::escape_xml(text_color);
    let mbox = layout_node(&root, font_size, &text_color, measure, x, baseline_y);

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

type Attrs = Vec<(String, String)>;

/// Read one TeX script argument without consuming the following postfix script.
fn script_argument(input: &str) -> Option<(&str, usize)> {
    let trimmed = input.trim_start();
    let leading = input.len() - trimmed.len();
    if trimmed.starts_with('{') {
        let mut depth = 0;
        let mut escaped = false;
        for (i, ch) in trimmed.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((&trimmed[1..i], leading + i + 1));
                    }
                }
                _ => {}
            }
        }
        None
    } else if let Some(command) = trimmed.strip_prefix('\\') {
        let len = command
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .count();
        let len = if len == 0 {
            command.chars().next()?.len_utf8()
        } else {
            len
        };
        Some((&trimmed[..1 + len], leading + 1 + len))
    } else {
        let ch = trimmed.chars().next()?;
        if matches!(ch, '}' | '^' | '_') {
            return None;
        }
        Some((&trimmed[..ch.len_utf8()], leading + ch.len_utf8()))
    }
}

fn latex_node(latex: &str, display: bool) -> Result<MathNode, String> {
    let latex = preprocess_latex(latex);
    let mut normalized = String::with_capacity(latex.len());
    let mut pairs = HashMap::new();
    let mut texts = HashMap::new();
    let mut prefix = "MarkieScriptSlot".to_string();
    while latex.contains(&prefix) {
        prefix.push('X');
    }
    let mut i = 0;
    while i < latex.len() {
        let remaining = &latex[i..];
        // Keep text outside the converter's math lexer, which drops spaces and
        // misparses digits in \text. Letter-only slots survive that lexer.
        if remaining.starts_with("\\text{")
            && let Some((text, consumed)) = script_argument(&remaining[5..])
        {
            let marker = format!("{prefix}Text{}", "X".repeat(texts.len() + 1));
            let mut text = text.to_string();
            for literal in ['&', '%', '$', '#', '_', '{', '}', ' '] {
                text = text.replace(&format!("\\{literal}"), &literal.to_string());
            }
            texts.insert(marker.clone(), text);
            normalized.push_str(&format!("\\text{{{marker}}}"));
            i += 5 + consumed;
            continue;
        }
        let ch = remaining.chars().next().unwrap();
        if ch == '\\'
            && let Some((_, consumed)) = script_argument(remaining)
        {
            // Consume commands and escaped literals as one token so \_ cannot
            // become a script operator during paired-script normalization.
            normalized.push_str(&remaining[..consumed]);
            i += consumed;
            continue;
        }
        if matches!(ch, '_' | '^')
            && let Some((first, consumed)) = script_argument(&remaining[1..])
        {
            let after = &remaining[1 + consumed..];
            let second = after.trim_start();
            if second.starts_with(if ch == '_' { '^' } else { '_' })
                && let Some((second_arg, second_len)) = script_argument(&second[1..])
            {
                // The converter incorrectly nests x_i^2 as x_{i^2}. Keep the
                // two source arguments separate without changing x_{i^2}.
                let (sub, sup) = if ch == '_' {
                    (first, second_arg)
                } else {
                    (second_arg, first)
                };
                let marker = format!("{prefix}Pair{}", "X".repeat(pairs.len() + 1));
                pairs.insert(
                    marker.clone(),
                    (latex_node(sub, display)?, latex_node(sup, display)?),
                );
                normalized.push_str(&format!("_{{\\text{{{marker}}}}}"));
                i += 1 + consumed + (after.len() - second.len()) + 1 + second_len;
                continue;
            }
        }
        normalized.push(ch);
        i += ch.len_utf8();
    }
    let style = if display {
        DisplayStyle::Block
    } else {
        DisplayStyle::Inline
    };
    let mut mathml =
        latex_to_mathml(&normalized, style).map_err(|e| format!("LaTeX parse error: {e:?}"))?;
    if mathml.contains("<mtext>[PARSE ERROR:") {
        return Err("LaTeX parse error: unsupported or invalid command".to_string());
    }
    // The converter emits bare operator text and unquoted column alignment.
    // Escape only token contents, leaving the generated MathML structure intact.
    for tag in ["mtext", "mi", "mn", "mo"] {
        let mut offset = 0;
        while let Some(start) = mathml[offset..].find(&format!("<{tag}")) {
            let start = offset + start;
            if !matches!(
                mathml.as_bytes().get(start + tag.len() + 1),
                Some(b' ' | b'>')
            ) {
                offset = start + tag.len() + 1;
                continue;
            }
            let Some(open_end) = mathml[start..].find('>') else {
                break;
            };
            let text_start = start + open_end + 1;
            let Some(end) = mathml[text_start..].find(&format!("</{tag}>")) else {
                break;
            };
            let text_end = text_start + end;
            let escaped = escape_xml(&mathml[text_start..text_end]);
            mathml.replace_range(text_start..text_end, &escaped);
            offset = text_start + escaped.len() + tag.len() + 3;
        }
    }
    mathml = mathml.replace("<mtable columnalign=left>", "<mtable columnalign=\"left\">");
    for (marker, text) in texts {
        mathml = mathml.replace(
            &format!("<mtext>{marker}</mtext>"),
            &format!("<mtext>{}</mtext>", escape_xml(&text)),
        );
    }
    parse_mathml(&mathml, &mut pairs)
}

fn parse_mathml(
    mathml: &str,
    pairs: &mut HashMap<String, (MathNode, MathNode)>,
) -> Result<MathNode, String> {
    let mut reader = XmlReader::from_str(mathml);
    reader.config_mut().trim_text(false);

    // Stack now stores: (tag_name, children, attributes)
    let mut stack: Vec<(String, Vec<MathNode>, Attrs)> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attrs: Attrs = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        (
                            String::from_utf8_lossy(a.key.as_ref()).to_string(),
                            String::from_utf8_lossy(&a.value).to_string(),
                        )
                    })
                    .collect();

                stack.push((name, Vec::new(), attrs));
            }
            Ok(XmlEvent::Text(ref e)) => {
                let text = e.decode().unwrap_or_default().to_string();
                if !text.is_empty()
                    && let Some((_, children, _)) = stack.last_mut()
                {
                    children.push(MathNode::Text(text));
                }
            }
            Ok(XmlEvent::GeneralRef(ref e)) => {
                let entity = e.decode().map_err(|e| format!("XML entity error: {e}"))?;
                let reference = format!("&{entity};");
                let text = quick_xml::escape::unescape(&reference)
                    .map_err(|e| format!("XML entity error: {e}"))?
                    .into_owned();
                if let Some((_, children, _)) = stack.last_mut() {
                    children.push(MathNode::Text(text));
                }
            }
            Ok(XmlEvent::End(ref e)) => {
                let _name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if let Some((tag, children, attrs)) = stack.pop() {
                    let node = build_node(&tag, children, &attrs, pairs);
                    if let Some((_, parent_children, _)) = stack.last_mut() {
                        parent_children.push(node);
                    } else {
                        return Ok(node);
                    }
                }
            }
            Ok(XmlEvent::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attrs: Attrs = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        (
                            String::from_utf8_lossy(a.key.as_ref()).to_string(),
                            String::from_utf8_lossy(&a.value).to_string(),
                        )
                    })
                    .collect();

                if name == "mspace" {
                    let mut width_em = 0.0;
                    for (key, val) in &attrs {
                        if key == "width"
                            && let Some(stripped) = val.strip_suffix("em")
                        {
                            width_em = stripped.parse().unwrap_or(0.0);
                        }
                    }
                    let node = MathNode::Space(width_em);
                    if let Some((_, parent_children, _)) = stack.last_mut() {
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

fn get_attr(attrs: &[(String, String)], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
}

fn build_node(
    tag: &str,
    mut children: Vec<MathNode>,
    attrs: &Attrs,
    pairs: &mut HashMap<String, (MathNode, MathNode)>,
) -> MathNode {
    if matches!(tag, "msub" | "munder")
        && children.len() == 2
        && let MathNode::Text(marker) = &children[1]
        && let Some((sub, sup)) = pairs.remove(marker)
    {
        let base = Box::new(children.remove(0));
        return if tag == "munder" {
            MathNode::UnderOver {
                base,
                under: Some(Box::new(sub)),
                over: Some(Box::new(sup)),
            }
        } else {
            MathNode::SubSup {
                base,
                sub: Box::new(sub),
                sup: Box::new(sup),
            }
        };
    }
    match tag {
        "mtd" | "mtr" => MathNode::Row(children),
        "mtable" => MathNode::Table {
            rows: children
                .into_iter()
                .map(|row| match row {
                    MathNode::Row(cells) => cells,
                    node => vec![node],
                })
                .collect(),
            column_align: get_attr(attrs, "columnalign")
                .unwrap_or_else(|| "center".to_string())
                .split_whitespace()
                .map(str::to_string)
                .collect(),
        },
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
            // Check for stretchy attribute
            let stretchy = get_attr(attrs, "stretchy")
                .map(|v| v == "true")
                .unwrap_or(false);
            let form = get_attr(attrs, "form").unwrap_or_else(|| "infix".to_string());

            if stretchy && !text.is_empty() {
                MathNode::StretchyOp { op: text, form }
            } else {
                MathNode::Operator(text)
            }
        }
        "mtext" => {
            let text = extract_text(&children);
            MathNode::Text(text)
        }
        "msup" if children.len() >= 2 => {
            let sup = children.pop().unwrap_or_default();
            let base = children.pop().unwrap_or_default();
            MathNode::Sup {
                base: Box::new(base),
                sup: Box::new(sup),
            }
        }
        "msub" if children.len() >= 2 => {
            let sub = children.pop().unwrap_or_default();
            let base = children.pop().unwrap_or_default();
            MathNode::Sub {
                base: Box::new(base),
                sub: Box::new(sub),
            }
        }
        "msubsup" if children.len() >= 3 => {
            let sup = children.pop().unwrap_or_default();
            let sub = children.pop().unwrap_or_default();
            let base = children.pop().unwrap_or_default();
            MathNode::SubSup {
                base: Box::new(base),
                sub: Box::new(sub),
                sup: Box::new(sup),
            }
        }
        "mfrac" if children.len() >= 2 => {
            let den = children.pop().unwrap_or_default();
            let num = children.pop().unwrap_or_default();
            // Check for linethickness attribute (used by binomial)
            let line_thickness = get_attr(attrs, "linethickness").and_then(|v| {
                if v == "0" {
                    Some(0.0)
                } else {
                    v.parse::<f32>().ok()
                }
            });
            MathNode::Frac {
                num: Box::new(num),
                den: Box::new(den),
                line_thickness,
            }
        }
        "msqrt" => {
            let radicand = if children.len() == 1 {
                children.pop().unwrap_or_default()
            } else {
                MathNode::Row(children)
            };
            MathNode::Sqrt {
                radicand: Box::new(radicand),
            }
        }
        "mroot" if children.len() >= 2 => {
            // Note: in MathML mroot, the index comes AFTER the radicand
            let index = children.pop().unwrap_or_default();
            let radicand = children.pop().unwrap_or_default();
            MathNode::Root {
                radicand: Box::new(radicand),
                index: Box::new(index),
            }
        }
        "mover" if children.len() >= 2 => {
            let over = children.pop().unwrap_or_default();
            let base = children.pop().unwrap_or_default();
            MathNode::UnderOver {
                base: Box::new(base),
                under: None,
                over: Some(Box::new(over)),
            }
        }
        "munder" if children.len() >= 2 => {
            let under = children.pop().unwrap_or_default();
            let base = children.pop().unwrap_or_default();
            MathNode::UnderOver {
                base: Box::new(base),
                under: Some(Box::new(under)),
                over: None,
            }
        }
        "munderover" if children.len() >= 3 => {
            let over = children.pop().unwrap_or_default();
            let under = children.pop().unwrap_or_default();
            let base = children.pop().unwrap_or_default();
            MathNode::UnderOver {
                base: Box::new(base),
                under: Some(Box::new(under)),
                over: Some(Box::new(over)),
            }
        }
        "math" | "mrow" | "mstyle" | "mpadded" => {
            if children.len() == 1 {
                children.pop().unwrap_or_default()
            } else {
                MathNode::Row(children)
            }
        }
        _ => {
            if children.len() == 1 {
                children.pop().unwrap_or_default()
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
    crate::xml::escape_xml(text)
}

fn measure_token<T: TextMeasure>(
    text: &str,
    font_size: f32,
    italic: bool,
    measure: &mut T,
) -> (f32, f32) {
    let cleaned = crate::xml::sanitize_xml_text(text);
    measure.measure_text(&cleaned, font_size, false, false, italic, None)
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
            let italic = text.len() == 1 && text.chars().next().is_some_and(|c| c.is_alphabetic());
            let (w, _h) = measure_token(text, font_size, italic, measure);
            let style = if italic { " font-style=\"italic\"" } else { "" };
            let svg = format!(
                r#"<text x="{:.2}" y="{:.2}" font-family="serif" font-size="{:.2}" fill="{}"{}>{}</text>"#,
                x,
                baseline_y,
                font_size,
                color,
                style,
                escape_xml(text)
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
                x,
                baseline_y,
                font_size,
                color,
                escape_xml(text)
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
            let effective_size = if is_large { font_size * 1.4 } else { font_size };
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
                r#"<text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="{:.2}" fill="{}" xml:space="preserve">{}</text>"#,
                x,
                baseline_y,
                font_size,
                color,
                escape_xml(text)
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
            let sup_box = layout_node(sup, sup_size, color, measure, x + base_box.width, sup_y);

            let total_width = base_box.width + sup_box.width;
            let ascent = base_box.ascent.max(sup_box.ascent + base_box.ascent * 0.55);
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
            let sub_box = layout_node(sub, sub_size, color, measure, x + base_box.width, sub_y);

            let total_width = base_box.width + sub_box.width;
            let ascent = base_box.ascent;
            let descent =
                (base_box.descent + sub_size * 0.35 + sub_box.descent).max(base_box.descent);

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
            let sup_box = layout_node(sup, script_size, color, measure, x + base_box.width, sup_y);

            let sub_y = baseline_y + base_box.descent + script_size * 0.35;
            let sub_box = layout_node(sub, script_size, color, measure, x + base_box.width, sub_y);

            let script_width = sup_box.width.max(sub_box.width);
            let total_width = base_box.width + script_width;
            let ascent = base_box.ascent.max(sup_box.ascent + base_box.ascent * 0.55);
            let descent =
                (base_box.descent + script_size * 0.35 + sub_box.descent).max(base_box.descent);

            MathBox {
                width: total_width,
                ascent,
                descent,
                svg: format!("{}{}{}", base_box.svg, sup_box.svg, sub_box.svg),
            }
        }
        MathNode::UnderOver { base, under, over } => layout_underover(
            base,
            under.as_deref(),
            over.as_deref(),
            &mut UnderoverContext {
                font_size,
                color,
                measure,
                x,
                baseline_y,
            },
        ),
        MathNode::Frac {
            num,
            den,
            line_thickness,
        } => {
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

            // Only draw line if line_thickness is not Some(0.0) (for binomials)
            let rule_svg = if line_thickness != &Some(0.0) {
                format!(
                    r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
                    x,
                    rule_y,
                    x + frac_width,
                    rule_y,
                    color
                )
            } else {
                String::new()
            };

            let ascent = (baseline_y - num_baseline + num_rendered.ascent).max(font_size * 0.75);
            let descent = (den_baseline - baseline_y + den_rendered.descent).max(font_size * 0.25);

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
        MathNode::Root { radicand, index } => {
            let inner = layout_node(radicand, font_size, color, measure, 0.0, 0.0);
            let index_size = font_size * 0.6;

            let radical_width = font_size * 0.6;
            let index_width = font_size * 0.5;
            let padding = font_size * 0.1;
            let overbar_gap = font_size * 0.15;
            let total_width = index_width + radical_width + inner.width + padding;

            let inner_box = layout_node(
                radicand,
                font_size,
                color,
                measure,
                x + index_width + radical_width,
                baseline_y,
            );

            let top_y = baseline_y - inner_box.ascent - overbar_gap;
            let bottom_y = baseline_y + inner_box.descent;

            // Render the index (nth root degree) in the notch
            let index_baseline = baseline_y - inner_box.ascent * 0.3;
            let index_box = layout_node(index, index_size, color, measure, x, index_baseline);

            // Radical symbol with notch for index
            let radical_svg = format!(
                r#"<path d="M {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2}" stroke="{}" stroke-width="1.2" fill="none" />"#,
                x + index_width,
                baseline_y - font_size * 0.15,
                x + index_width + radical_width * 0.35,
                baseline_y,
                x + index_width + radical_width * 0.6,
                top_y,
                x + index_width + radical_width + inner_box.width + padding,
                top_y,
                x + index_width + radical_width + inner_box.width + padding - font_size * 0.1,
                top_y - font_size * 0.05,
                color
            );

            let ascent = (baseline_y - top_y).max(inner_box.ascent + overbar_gap);
            let descent = inner_box.descent.max(bottom_y - baseline_y);

            MathBox {
                width: total_width,
                ascent,
                descent,
                svg: format!("{}{}{}", index_box.svg, radical_svg, inner_box.svg),
            }
        }
        MathNode::Table { rows, column_align } => {
            layout_table(rows, column_align, font_size, color, measure, x, baseline_y)
        }
        MathNode::StretchyOp { op, form } => {
            let (w, _h) = measure_token(op, font_size, false, measure);
            let offset = match form.as_str() {
                "prefix" => font_size * 0.08,
                "postfix" => -font_size * 0.08,
                _ => 0.0,
            };
            let svg = format!(
                r#"<text x="{:.2}" y="{:.2}" font-family="serif" font-size="{:.2}" fill="{}">{}</text>"#,
                x + offset,
                baseline_y,
                font_size,
                color,
                escape_xml(op)
            );
            MathBox {
                width: w,
                ascent: font_size * 0.75,
                descent: font_size * 0.25,
                svg,
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
        if !matches!(child, MathNode::StretchyOp { .. }) {
            let child_box = layout_node(child, font_size, color, measure, 0.0, 0.0);
            max_ascent = max_ascent.max(child_box.ascent);
            max_descent = max_descent.max(child_box.descent);
        }
    }
    for child in children {
        let child_box = if matches!(child, MathNode::StretchyOp { .. }) {
            let size = max_ascent + max_descent;
            let baseline = baseline_y + size * 0.75 - max_ascent;
            layout_node(child, size, color, measure, cx, baseline)
        } else {
            layout_node(child, font_size, color, measure, cx, baseline_y)
        };
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

struct UnderoverContext<'a, T: TextMeasure> {
    font_size: f32,
    color: &'a str,
    measure: &'a mut T,
    x: f32,
    baseline_y: f32,
}

fn layout_underover<T: TextMeasure>(
    base: &MathNode,
    under: Option<&MathNode>,
    over: Option<&MathNode>,
    ctx: &mut UnderoverContext<'_, T>,
) -> MathBox {
    let font_size = ctx.font_size;
    let color = ctx.color;
    let measure = &mut *ctx.measure;
    let x = ctx.x;
    let baseline_y = ctx.baseline_y;

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
        let over_rendered = layout_node(
            over_node,
            script_size,
            color,
            measure,
            over_x,
            over_baseline,
        );
        svg.push_str(&over_rendered.svg);
        total_ascent = base_box.ascent + gap + ob.ascent + ob.descent;
    }

    if let (Some(under_node), Some(ub)) = (under, &under_box) {
        let under_baseline = baseline_y + base_box.descent + gap + ub.ascent;
        let under_x = x + (max_width - ub.width) / 2.0;
        let under_rendered = layout_node(
            under_node,
            script_size,
            color,
            measure,
            under_x,
            under_baseline,
        );
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

fn layout_table<T: TextMeasure>(
    rows: &[Vec<MathNode>],
    column_align: &[String],
    font_size: f32,
    color: &str,
    measure: &mut T,
    x: f32,
    baseline_y: f32,
) -> MathBox {
    if rows.is_empty() {
        return MathBox {
            width: 0.0,
            ascent: font_size * 0.75,
            descent: font_size * 0.25,
            svg: String::new(),
        };
    }

    let cell_size = font_size * 0.9;
    let row_gap = font_size * 0.3;
    let col_gap = font_size * 0.4;

    // First pass: measure all cells to determine column widths and row heights
    let mut col_widths: Vec<f32> = Vec::new();
    let mut row_heights: Vec<(f32, f32)> = Vec::new(); // (ascent, descent) per row

    for row in rows {
        let mut row_ascent = cell_size * 0.75;
        let mut row_descent = cell_size * 0.25;

        for (col_idx, cell) in row.iter().enumerate() {
            let cell_box = layout_node(cell, cell_size, color, measure, 0.0, 0.0);

            // Expand column width if needed
            while col_widths.len() <= col_idx {
                col_widths.push(0.0);
            }
            col_widths[col_idx] = col_widths[col_idx].max(cell_box.width);

            row_ascent = row_ascent.max(cell_box.ascent);
            row_descent = row_descent.max(cell_box.descent);
        }
        row_heights.push((row_ascent, row_descent));
    }

    // Calculate total table dimensions
    let total_width: f32 =
        col_widths.iter().sum::<f32>() + col_gap * (col_widths.len().max(1) - 1) as f32;
    let total_height: f32 =
        row_heights.iter().map(|(a, d)| a + d).sum::<f32>() + row_gap * (rows.len() - 1) as f32;

    // Center the table vertically around baseline
    let table_top = baseline_y - total_height / 2.0;

    // Second pass: render all cells
    let mut svg = String::new();
    let mut current_y = table_top;

    for (row_idx, row) in rows.iter().enumerate() {
        let (row_ascent, row_descent) = row_heights[row_idx];
        let row_baseline = current_y + row_ascent;
        let mut current_x = x;

        for (col_idx, cell) in row.iter().enumerate() {
            let col_width = col_widths.get(col_idx).copied().unwrap_or(0.0);
            let cell_box = layout_node(cell, cell_size, color, measure, 0.0, 0.0);

            let alignment = column_align.get(col_idx).or_else(|| column_align.last());
            let cell_x = current_x
                + match alignment.map(String::as_str) {
                    Some("left") => 0.0,
                    Some("right") => col_width - cell_box.width,
                    _ => (col_width - cell_box.width) / 2.0,
                };

            let rendered = layout_node(cell, cell_size, color, measure, cell_x, row_baseline);
            svg.push_str(&rendered.svg);

            current_x += col_width + col_gap;
        }

        current_y += row_ascent + row_descent + row_gap;
    }

    MathBox {
        width: total_width,
        ascent: total_height / 2.0,
        descent: total_height / 2.0,
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
            (text.len() as f32 * font_size * 0.6, font_size)
        }
    }

    #[test]
    fn test_extract_text() {
        // Scenario 1: Empty input
        assert_eq!(extract_text(&[]), "");

        // Scenario 2: Supported variants
        let children = vec![
            MathNode::Text("Hello ".to_string()),
            MathNode::Ident("x".to_string()),
            MathNode::Number("123".to_string()),
            MathNode::Operator("+".to_string()),
        ];
        assert_eq!(extract_text(&children), "Hello x123+");

        // Scenario 3: Ignored variants
        let ignored = vec![
            MathNode::Row(vec![]),
            MathNode::Sup {
                base: Box::new(MathNode::Text("b".to_string())),
                sup: Box::new(MathNode::Text("s".to_string())),
            },
            MathNode::Sub {
                base: Box::new(MathNode::Text("b".to_string())),
                sub: Box::new(MathNode::Text("s".to_string())),
            },
            MathNode::SubSup {
                base: Box::new(MathNode::Text("b".to_string())),
                sub: Box::new(MathNode::Text("s".to_string())),
                sup: Box::new(MathNode::Text("p".to_string())),
            },
            MathNode::Frac {
                num: Box::new(MathNode::Text("n".to_string())),
                den: Box::new(MathNode::Text("d".to_string())),
                line_thickness: None,
            },
            MathNode::Sqrt {
                radicand: Box::new(MathNode::Text("r".to_string())),
            },
            MathNode::Root {
                radicand: Box::new(MathNode::Text("r".to_string())),
                index: Box::new(MathNode::Text("i".to_string())),
            },
            MathNode::UnderOver {
                base: Box::new(MathNode::Text("b".to_string())),
                under: Some(Box::new(MathNode::Text("u".to_string()))),
                over: Some(Box::new(MathNode::Text("o".to_string()))),
            },
            MathNode::Space(1.0),
            MathNode::Table {
                rows: vec![],
                column_align: vec![],
            },
            MathNode::StretchyOp {
                op: "(".to_string(),
                form: "prefix".to_string(),
            },
        ];
        assert_eq!(extract_text(&ignored), "");

        // Scenario 4: Mixture
        let mixture = vec![
            MathNode::Text("val: ".to_string()),
            MathNode::Space(2.0),
            MathNode::Number("42".to_string()),
            MathNode::Row(vec![MathNode::Text("inner".to_string())]),
        ];
        assert_eq!(extract_text(&mixture), "val: 42");
    }

    #[test]
    fn test_render_math_basic() {
        let mut measure = MockMeasure;
        let result = render_math("x + 1", 16.0, "#000000", &mut measure, false);

        assert!(result.is_ok());
        let math_res = result.unwrap();
        assert!(math_res.width > 0.0);
        assert!(math_res.ascent > 0.0);
        assert!(math_res.descent > 0.0);
        assert!(math_res.svg_fragment.contains("<text"));
        assert!(math_res.svg_fragment.contains("x"));
        assert!(math_res.svg_fragment.contains("+"));
        assert!(math_res.svg_fragment.contains("1"));
    }

    #[test]
    fn test_render_math_error() {
        let mut measure = MockMeasure;
        for latex in [r"\frac{a}{", r"\definitelyunknown{x}"] {
            let result = render_math(latex, 16.0, "#000000", &mut measure, false);
            assert!(result.unwrap_err().contains("LaTeX parse error"));
        }
    }

    #[test]
    fn render_math_escapes_color_attributes_without_restricting_css_colors() {
        let mut measure = MockMeasure;
        for color in [
            "#123456\" injected=\"yes",
            "rebeccapurple",
            "rgb(12, 34, 56)",
        ] {
            let rendered = render_math(r"\frac{x}{y}", 16.0, color, &mut measure, false).unwrap();
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">{}</svg>"#,
                rendered.svg_fragment
            );
            let mut reader = quick_xml::Reader::from_str(&svg);
            let mut fills = 0;
            let mut strokes = 0;
            loop {
                match reader.read_event().unwrap() {
                    quick_xml::events::Event::Start(node)
                    | quick_xml::events::Event::Empty(node) => {
                        for attribute in node.attributes() {
                            let attribute = attribute.unwrap();
                            assert_ne!(attribute.key.as_ref(), b"injected");
                            match attribute.key.as_ref() {
                                b"fill" | b"stroke" => {
                                    assert_eq!(
                                        attribute
                                            .decode_and_unescape_value(reader.decoder())
                                            .unwrap(),
                                        color
                                    );
                                    if attribute.key.as_ref() == b"fill" {
                                        fills += 1;
                                    } else {
                                        strokes += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    quick_xml::events::Event::Eof => break,
                    _ => {}
                }
            }
            assert!(fills > 0 && strokes > 0);
        }
    }

    #[test]
    fn render_math_preserves_text_and_nested_matrix_cells() {
        let mut measure = MockMeasure;
        for (latex, tokens) in [
            ("α+β+漢", vec!["α", "β", "漢"]),
            (r"\_a^b", vec![">_</text>", ">a</text>", ">b</text>"]),
            (r"\text{hello 17 < 3 \& x}", vec!["hello 17 &lt; 3 &amp; x"]),
            (
                r"\begin{matrix}1&\begin{matrix}2&3\\5&7\end{matrix}\\9&4\end{matrix}",
                vec![
                    ">1</text>",
                    ">2</text>",
                    ">3</text>",
                    ">5</text>",
                    ">7</text>",
                    ">9</text>",
                    ">4</text>",
                ],
            ),
        ] {
            let rendered = render_math(latex, 16.0, "#000000", &mut measure, true).unwrap();
            for token in tokens {
                assert!(
                    rendered.svg_fragment.contains(token),
                    "lost {token} in {latex}"
                );
            }
        }
    }

    #[test]
    fn test_render_math_display_mode() {
        let mut measure = MockMeasure;
        let inline_res = render_math("x^2", 16.0, "#000000", &mut measure, false).unwrap();
        let display_res = render_math("x^2", 16.0, "#000000", &mut measure, true).unwrap();

        // They might have different metrics or structures, but both should be valid SVGs
        assert!(inline_res.svg_fragment.contains("<text"));
        assert!(display_res.svg_fragment.contains("<text"));
    }

    #[test]
    fn test_render_math_complex() {
        let mut measure = MockMeasure;
        let expressions = vec![
            "\\frac{a}{b}",
            "\\sqrt{x}",
            "\\sqrt[3]{x}",
            "\\sum_{i=0}^n i",
            "x_{i}",
            "x^{2}",
            "x_{i}^{2}",
            "\\text{plain text}",
            "\\begin{matrix} a & b \\\\ c & d \\end{matrix}",
            "\\binom{n}{k}",
        ];

        for expr in expressions {
            let result = render_math(expr, 16.0, "#000000", &mut measure, true);
            assert!(
                result.is_ok(),
                "Failed to render complex expression: {}",
                expr
            );
            let res = result.unwrap();
            assert!(res.width > 0.0);
            assert!(!res.svg_fragment.is_empty());
        }
    }

    #[test]
    fn test_render_aligned_environment() {
        let mut measure = MockMeasure;
        let latex = r"\begin{aligned} a &= b + c \\ d &= e + f \end{aligned}";
        let result = render_math(latex, 16.0, "#000000", &mut measure, true);
        assert!(
            result.is_ok(),
            "aligned environment should render successfully, got: {:?}",
            result.err()
        );
        let res = result.unwrap();
        assert!(res.width > 0.0);
    }

    #[test]
    fn test_render_cases_environment() {
        let mut measure = MockMeasure;
        let latex = r"\begin{cases} x + y = 1 \\ x - y = 0 \end{cases}";
        let result = render_math(latex, 16.0, "#000000", &mut measure, true);
        assert!(
            result.is_ok(),
            "cases environment should render successfully, got: {:?}",
            result.err()
        );
        let res = result.unwrap();
        assert!(res.width > 0.0);
    }

    #[test]
    fn test_render_array_environment() {
        let mut measure = MockMeasure;
        let latex = r"\begin{array}{cc} 1 & 2 \\ 3 & 4 \end{array}";
        let result = render_math(latex, 16.0, "#000000", &mut measure, true);
        assert!(
            result.is_ok(),
            "array environment should render successfully, got: {:?}",
            result.err()
        );
        let res = result.unwrap();
        assert!(res.width > 0.0);
    }

    #[test]
    fn test_preprocess_preserves_supported_environments() {
        let mut measure = MockMeasure;
        let latex = r"\begin{bmatrix} a & b \\ c & d \end{bmatrix}";
        let result = render_math(latex, 16.0, "#000000", &mut measure, true);
        assert!(
            result.is_ok(),
            "bmatrix should still work after preprocessing"
        );
    }
}

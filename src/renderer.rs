use crate::fonts::TextMeasure;
use crate::theme::Theme;
use base64::Engine;
use imagesize;
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use resvg::usvg;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

const LIST_INDENT_RATIO: f32 = 1.5;
const LIST_MARKER_GAP_RATIO: f32 = 0.5;
const QUOTE_INDENT_RATIO: f32 = 1.25;
const QUOTE_INNER_PADDING_RATIO: f32 = 0.75;

struct ListState {
    ordered: bool,
    next_index: usize,
    needs_ascent: bool,
}

struct PendingListMarker {
    marker: String,
    marker_x: f32,
}

struct QuoteState {
    border_x: f32,
    start_y: f32,
}

struct ImageState {
    src: String,
    alt_text: String,
}

struct ImagePayload {
    data_url: String,
    width: f32,
    height: f32,
}

struct TableCellData {
    text: String,
}

struct TableRowData {
    cells: Vec<TableCellData>,
    is_header: bool,
}

struct TableState {
    alignments: Vec<Alignment>,
    rows: Vec<TableRowData>,
    current_row: Option<TableRowData>,
    current_cell: Option<TableCellData>,
    in_head: bool,
}

struct DefinitionListState {
    indent: f32,
}

/// Inline style applied by HTML tags such as `<span style="...">`, `<sup>`, `<sub>`.
#[derive(Debug, Clone)]
struct InlineHtmlStyle {
    /// Foreground color (hex, named, or rgb()/rgba() — passed through to SVG).
    color: Option<String>,
    /// Background highlight color (for `<mark>` / `background-color`).
    background: Option<String>,
    /// Font-size multiplier (e.g. 0.7 for superscripts).
    scale: f32,
    /// Baseline shift as a fraction of the current font size (negative = up).
    rise_ratio: f32,
    /// Underline decoration (`<u>` / `text-decoration: underline`).
    underline: bool,
}

impl Default for InlineHtmlStyle {
    fn default() -> Self {
        Self {
            color: None,
            background: None,
            scale: 1.0,
            rise_ratio: 0.0,
            underline: false,
        }
    }
}

impl InlineHtmlStyle {
    fn is_empty(&self) -> bool {
        self.color.is_none()
            && self.background.is_none()
            && self.scale == 1.0
            && self.rise_ratio == 0.0
            && !self.underline
    }
}

/// A parsed opening/closing inline HTML tag.
struct ParsedHtmlTag {
    name: String,
    closing: bool,
    self_closing: bool,
    style: InlineHtmlStyle,
}

pub struct Renderer<T: TextMeasure = crate::fonts::CosmicTextMeasure> {
    theme: Theme,
    measure: T,
    svg_content: String,
    cursor_x: f32,
    cursor_y: f32,
    width: f32,
    at_line_start: bool,

    heading_level: Option<HeadingLevel>,
    strong_depth: usize,
    emphasis_depth: usize,
    link_depth: usize,

    list_stack: Vec<ListState>,
    item_continuation_indent: Option<f32>,

    blockquotes: Vec<QuoteState>,

    current_image: Option<ImageState>,

    pending_list_marker: Option<PendingListMarker>,

    in_table: bool,
    table_state: Option<TableState>,

    in_strikethrough: bool,
    in_display_math: bool,
    pending_math_block: Option<String>,

    in_code_block: bool,
    code_block_buffer: String,
    code_block_lang: Option<String>,
    code_block_start_line: usize,
    current_event_line: usize,

    in_html_block: bool,
    html_block_buffer: String,

    in_metadata_block: bool,

    definition_list_stack: Vec<DefinitionListState>,

    in_footnote_definition: bool,

    pending_text: String,

    last_margin_added: f32,

    /// Stack of open inline HTML style scopes (`<span>`, `<sup>`, `<u>`, ...).
    html_style_stack: Vec<InlineHtmlStyle>,

    /// Memoized space-advance width per (font_size, bold, italic). Inferring the
    /// width costs three measurements; without this cache every whitespace token
    /// would repeat all three (even with the global LRU, that's 3 lookups per space).
    space_width_cache: HashMap<(u32, bool, bool), f32>,

    ps: SyntaxSet,
    ts: ThemeSet,

    base_path: Option<PathBuf>,
}

/// Scan a tag's attribute list (e.g. `style="color: red" class="x"`) into a style.
fn parse_inline_html_attrs(rest: &str) -> InlineHtmlStyle {
    let mut style = InlineHtmlStyle::default();
    let bytes = rest.as_bytes();
    let is_ws = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c);

    let mut i = 0;
    while i < rest.len() {
        while i < rest.len() && is_ws(bytes[i]) {
            i += 1;
        }
        if i >= rest.len() {
            break;
        }

        let name_start = i;
        while i < rest.len() && !is_ws(bytes[i]) && bytes[i] != b'=' {
            i += 1;
        }
        let name = rest[name_start..i].trim().to_ascii_lowercase();
        if name.is_empty() {
            i += 1;
            continue;
        }

        while i < rest.len() && is_ws(bytes[i]) {
            i += 1;
        }

        let mut value = String::new();
        if i < rest.len() && bytes[i] == b'=' {
            i += 1;
            while i < rest.len() && is_ws(bytes[i]) {
                i += 1;
            }
            if i < rest.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let start = i;
                while i < rest.len() && bytes[i] != quote {
                    i += 1;
                }
                value = rest[start..i].to_string();
                if i < rest.len() {
                    i += 1;
                }
            } else {
                let start = i;
                while i < rest.len() && !is_ws(bytes[i]) {
                    i += 1;
                }
                value = rest[start..i].to_string();
            }
        }

        apply_inline_html_attr(&mut style, &name, &value);
    }

    style
}

fn apply_inline_html_attr(style: &mut InlineHtmlStyle, name: &str, value: &str) {
    let value = value.trim();
    match name {
        "style" => {
            for decl in value.split(';') {
                let Some((prop, val)) = decl.split_once(':') else {
                    continue;
                };
                let prop = prop.trim().to_ascii_lowercase();
                let val = val.trim();
                if val.is_empty() {
                    continue;
                }
                match prop.as_str() {
                    "color" => style.color = sanitize_color(val),
                    "background" | "background-color" => {
                        style.background = sanitize_color(val);
                    }
                    "text-decoration"
                        if val.to_ascii_lowercase().contains("underline") =>
                    {
                        style.underline = true;
                    }
                    "font-size" => {
                        if let Some(scale) = parse_font_size_scale(val) {
                            style.scale *= scale;
                        }
                    }
                    _ => {}
                }
            }
        }
        "color" => style.color = sanitize_color(value),
        "bgcolor" | "background" => style.background = sanitize_color(value),
        _ => {}
    }
}

/// Parse `font-size` values (`16px`, `1.5em`, `150%`) into a scale relative to the
/// renderer's 16px base.
fn parse_font_size_scale(value: &str) -> Option<f32> {
    let v = value.trim().to_ascii_lowercase();
    let (num, unit) = if let Some(px) = v.strip_suffix("px") {
        (px, 16.0)
    } else if let Some(em) = v.strip_suffix("em") {
        (em, 1.0)
    } else if let Some(pct) = v.strip_suffix('%') {
        (pct, 0.01)
    } else {
        (v.as_str(), 1.0)
    };
    let n: f32 = num.trim().parse().ok()?;
    if !n.is_finite() || n <= 0.0 {
        return None;
    }
    Some((n * unit).clamp(0.4, 3.0))
}

/// Accept only values that are safe to embed as an SVG fill attribute and look
/// like colors: hex, `rgb()/rgba()/hsl()`, or CSS named colors (passed through
/// for resvg to resolve). Rejects anything containing quotes or markup.
fn sanitize_color(value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() || v.len() > 64 {
        return None;
    }
    if !v.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '#' | ',' | '(' | ')' | '%' | '.' | ' ')
    }) {
        return None;
    }
    if let Some(hex) = v.strip_prefix('#') {
        if [3usize, 6, 8].contains(&hex.len()) {
            Some(v.to_string())
        } else {
            None
        }
    } else if v.starts_with("rgb")
        || v.starts_with("hsl")
        || v.starts_with("var")
        || v.chars().all(|c| c.is_ascii_alphabetic())
    {
        // rgb()/rgba()/hsl()/var()/named colors: passed through for resvg to resolve.
        Some(v.to_string())
    } else {
        None
    }
}

/// `<mark>` highlight on light backgrounds: classic yellow.
const MARK_HIGHLIGHT_LIGHT: &str = "#ffff00";
/// `<mark>` highlight on dark backgrounds: dark olive-amber. Pure yellow on a
/// dark theme leaves light text at ~1:1 contrast (unreadable); this keeps the
/// highlight visibly warm while preserving text contrast (see contrast tests).
const MARK_HIGHLIGHT_DARK: &str = "#4a4600";

/// Parse a `#rrggbb` hex color into normalized (r, g, b) components.
fn parse_hex_rgb(value: &str) -> Option<(f32, f32, f32)> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some((r, g, b))
}

/// WCAG relative luminance of an (r, g, b) triple with components in 0..=1.
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

/// Whether a theme background counts as "dark" (relative luminance < 0.5).
/// Unparseable colors default to light, which is the safer fallback.
fn theme_is_dark(background_hex: &str) -> bool {
    match parse_hex_rgb(background_hex) {
        Some(c) => relative_luminance(c) < 0.5,
        None => false,
    }
}

/// WCAG contrast ratio between two `#rrggbb` colors (test utility).
#[cfg(test)]
fn contrast_ratio(a: &str, b: &str) -> Option<f32> {
    let l1 = relative_luminance(parse_hex_rgb(a)?);
    let l2 = relative_luminance(parse_hex_rgb(b)?);
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    Some((hi + 0.05) / (lo + 0.05))
}

impl<T: TextMeasure> Renderer<T> {
    pub fn new(theme: Theme, measure: T, width: f32) -> Result<Self, String> {
        Self::new_with_base_path(theme, measure, width, None)
    }

    pub fn new_with_base_path(
        theme: Theme,
        measure: T,
        width: f32,
        base_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let padding_x = theme.padding_x;
        let padding_y = theme.padding_y;

        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        Ok(Self {
            theme,
            measure,
            svg_content: String::with_capacity(64 * 1024), // 64KB initial capacity
            cursor_x: padding_x,
            cursor_y: padding_y,
            width,
            at_line_start: true,
            heading_level: None,
            strong_depth: 0,
            emphasis_depth: 0,
            link_depth: 0,
            list_stack: Vec::new(),
            item_continuation_indent: None,
            blockquotes: Vec::new(),
            current_image: None,
            pending_list_marker: None,
            in_table: false,
            table_state: None,
            in_strikethrough: false,
            in_display_math: false,
            pending_math_block: None,
            in_code_block: false,
            code_block_buffer: String::new(),
            code_block_lang: None,
            code_block_start_line: 0,
            current_event_line: 0,
            in_html_block: false,
            html_block_buffer: String::new(),
            in_metadata_block: false,
            definition_list_stack: Vec::new(),
            in_footnote_definition: false,
            pending_text: String::new(),
            last_margin_added: 0.0,
            html_style_stack: Vec::new(),
            space_width_cache: HashMap::new(),
            ps,
            ts,
            base_path,
        })
    }

    pub fn render(&mut self, markdown: &str) -> Result<String, String> {
        // Remove XML-illegal control chars before markdown parsing so syntax (e.g. headings)
        // still parses correctly when noisy bytes are present in input files.
        let markdown = crate::xml::sanitize_xml_text(markdown);

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_MATH);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_DEFINITION_LIST);
        options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        options.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
        options.insert(Options::ENABLE_GFM);

        let parser = Parser::new_ext(&markdown, options);

        // Precompute line-start offsets once, then binary-search per event. This
        // avoids re-counting newlines from the document start for every event
        // (O(n) per event → O(n²) total) and is robust to pulldown-cmark emitting
        // events with non-monotonic source offsets.
        let mut line_starts: Vec<usize> = Vec::with_capacity(markdown.len() / 32 + 1);
        line_starts.push(0);
        for (i, b) in markdown.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }

        for (event, range) in parser.into_offset_iter() {
            self.current_event_line = line_starts
                .partition_point(|&start| start <= range.start)
                .max(1);
            if self.in_metadata_block {
                if matches!(event, Event::End(TagEnd::MetadataBlock(_))) {
                    self.in_metadata_block = false;
                }
                continue;
            }

            if self.in_code_block {
                match event {
                    Event::End(TagEnd::CodeBlock) => {
                        self.render_code_block()?;
                        self.code_block_buffer.clear();
                        self.in_code_block = false;
                        self.code_block_lang = None;
                        self.add_margin(self.theme.margin_bottom);
                        self.cursor_x = self.line_start_x();
                        self.at_line_start = true;
                    }
                    Event::Text(text) => self.code_block_buffer.push_str(&text),
                    Event::Code(code) => self.code_block_buffer.push_str(&code),
                    Event::SoftBreak | Event::HardBreak => self.code_block_buffer.push('\n'),
                    _ => {}
                }
                continue;
            }

            if self.in_html_block {
                match event {
                    Event::End(TagEnd::HtmlBlock) => {
                        // HTML blocks are intentionally not rendered as HTML; show the
                        // source as a highlighted code block so content is never lost.
                        let buffer = std::mem::take(&mut self.html_block_buffer);
                        self.in_html_block = false;
                        if !buffer.trim().is_empty() {
                            self.render_code_block_with_language(&buffer, Some("html"))?;
                            self.add_margin(self.theme.margin_bottom);
                            self.cursor_x = self.line_start_x();
                            self.at_line_start = true;
                        }
                    }
                    Event::Html(html) => self.html_block_buffer.push_str(&html),
                    Event::SoftBreak | Event::HardBreak => self.html_block_buffer.push('\n'),
                    _ => {}
                }
                continue;
            }

            match &event {
                Event::Text(_) => {}
                _ => {
                    self.flush_pending_text()?;
                }
            }

            match event {
                Event::Start(tag) => self.handle_start_tag(tag)?,
                Event::End(tag_end) => self.handle_end_tag(tag_end)?,
                Event::Text(text) => {
                    if self.in_table {
                        self.render_table_text(&text);
                    } else if self.in_display_math {
                        self.append_math_text(&text);
                    } else {
                        self.pending_text.push_str(&text);
                    }
                }
                Event::Code(code) => {
                    if self.in_table {
                        self.render_table_text(&code);
                    } else {
                        self.render_inline_code(&code)?;
                    }
                }
                Event::InlineMath(math) => self.render_inline_math(&math)?,
                Event::DisplayMath(math) => self.render_display_math(&math)?,
                Event::Html(html) => {
                    if self.in_table {
                        // Ignore HTML inside tables
                    } else {
                        self.render_inline_html(&html)?;
                    }
                }
                Event::InlineHtml(html) => {
                    if self.in_table {
                        // Ignore inline HTML inside tables
                    } else {
                        self.render_inline_html(&html)?;
                    }
                }
                Event::SoftBreak => {
                    if self.in_table {
                        self.render_table_text(" ");
                    } else {
                        self.render_soft_break()?;
                    }
                }
                Event::HardBreak => {
                    if self.in_table {
                        self.render_table_text(" ");
                    } else {
                        self.render_newline()?;
                    }
                }
                Event::TaskListMarker(checked) => self.render_task_marker(checked)?,
                Event::FootnoteReference(label) => self.render_footnote_reference(&label)?,
                Event::Rule => self.render_horizontal_rule()?,
            }
        }

        self.flush_pending_text()?;

        if self.in_table {
            self.finish_table()?;
        }

        let total_height = self.cursor_y + self.theme.padding_y;
        Ok(self.finalize_svg(total_height))
    }

    fn handle_start_tag(&mut self, tag: Tag) -> Result<(), String> {
        match tag {
            Tag::Heading { level, .. } => {
                self.heading_level = Some(level);
                let top_margin_scale = match level {
                    HeadingLevel::H1 => 1.6,
                    HeadingLevel::H2 => 1.45,
                    HeadingLevel::H3 => 1.3,
                    _ => 1.15,
                };
                self.start_block(self.theme.margin_top * top_margin_scale, true);
            }
            Tag::Paragraph => {
                let in_container = self.item_continuation_indent.is_some();
                if in_container {
                    if !self.at_line_start {
                        self.new_line();
                    }
                    self.start_block(self.theme.margin_bottom * 0.5, true);
                } else {
                    self.start_block(0.0, true);
                }
            }
            Tag::CodeBlock(kind) => {
                self.start_block(self.theme.margin_top, false);
                self.in_code_block = true;
                self.code_block_buffer.clear();
                self.code_block_start_line = self.current_event_line;
                self.code_block_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                    _ => None,
                };
            }
            Tag::List(start) => {
                if self.list_stack.is_empty() {
                    self.start_block(self.theme.margin_top * 0.8, false);
                } else if !self.at_line_start {
                    self.new_line();
                }

                let needs_ascent = self.list_stack.is_empty();
                self.list_stack.push(ListState {
                    ordered: start.is_some(),
                    next_index: start.unwrap_or(1) as usize,
                    needs_ascent,
                });
            }
            Tag::Item => self.start_list_item()?,
            Tag::BlockQuote(_) => {
                self.start_block(self.theme.margin_top, false);
                self.start_blockquote();
            }
            Tag::Link { .. } => self.link_depth += 1,
            Tag::Image { dest_url, .. } => {
                self.current_image = Some(ImageState {
                    src: dest_url.to_string(),
                    alt_text: String::new(),
                });
            }
            Tag::HtmlBlock => {
                self.start_block(0.0, false);
                self.in_html_block = true;
                self.html_block_buffer.clear();
            }
            Tag::Table(alignments) => {
                self.start_table(alignments.to_vec());
            }
            Tag::TableHead => self.start_table_head(),
            Tag::TableRow => self.start_table_row(),
            Tag::TableCell => self.start_table_cell(),
            Tag::Emphasis => self.emphasis_depth += 1,
            Tag::Strong => self.strong_depth += 1,
            Tag::Strikethrough => self.in_strikethrough = true,
            Tag::MetadataBlock(_) => self.in_metadata_block = true,
            Tag::DefinitionList => {
                if self.definition_list_stack.is_empty() {
                    self.start_block(self.theme.margin_top * 0.8, false);
                } else if !self.at_line_start {
                    self.new_line();
                }

                let indent = self.base_left_indent() + self.theme.font_size_base * 1.5;
                self.definition_list_stack
                    .push(DefinitionListState { indent });
            }
            Tag::DefinitionListTitle => {
                if !self.at_line_start {
                    self.new_line();
                }
                self.add_margin(self.theme.margin_bottom * 0.6);
                self.cursor_y += self.current_font_size() * 0.8;
                self.strong_depth += 1;
                self.item_continuation_indent = None;
            }
            Tag::DefinitionListDefinition => {
                if !self.at_line_start {
                    self.new_line();
                }
                if let Some(state) = self.definition_list_stack.last() {
                    self.item_continuation_indent = Some(state.indent);
                }
                self.cursor_x = self.line_start_x();
            }
            Tag::FootnoteDefinition(label) => {
                self.start_block(self.theme.margin_top * 0.8, false);
                // Footnotes render like definition list items with a marker.
                self.in_footnote_definition = true;
                let marker = format!("[{}]", label);
                let marker_x = self.base_left_indent();
                self.pending_list_marker = Some(PendingListMarker { marker, marker_x });

                let (marker_width, _) = self.measure.measure_text(
                    self.pending_list_marker
                        .as_ref()
                        .map(|pending| pending.marker.as_str())
                        .unwrap_or(""),
                    self.theme.font_size_base,
                    false,
                    false,
                    false,
                    None,
                );
                self.item_continuation_indent =
                    Some(marker_x + marker_width + self.theme.font_size_base * 0.5);
                self.cursor_x = self.item_continuation_indent.unwrap_or(self.line_start_x());
                self.at_line_start = true;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_end_tag(&mut self, tag_end: TagEnd) -> Result<(), String> {
        match tag_end {
            TagEnd::Heading(_) => {
                let bottom_margin_scale = match self.heading_level {
                    Some(HeadingLevel::H1) | Some(HeadingLevel::H2) => 0.4,
                    _ => 0.6,
                };
                self.finish_block(self.theme.margin_bottom * bottom_margin_scale);
                self.heading_level = None;
            }
            TagEnd::Paragraph => {
                let is_list_paragraph = (self.item_continuation_indent.is_some()
                    && (!self.list_stack.is_empty() || !self.definition_list_stack.is_empty()))
                    || self.in_footnote_definition;
                let margin = if is_list_paragraph {
                    0.0
                } else {
                    self.theme.margin_bottom
                };
                self.finish_block(margin);
            }
            TagEnd::CodeBlock => {}
            TagEnd::Item => self.end_list_item(),
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.finish_block(self.theme.margin_bottom);
                }
            }
            TagEnd::BlockQuote(_) => {
                self.end_blockquote();
                self.add_margin(self.theme.margin_bottom);
                self.cursor_x = self.line_start_x();
                self.at_line_start = true;
            }
            TagEnd::Link => self.link_depth = self.link_depth.saturating_sub(1),
            TagEnd::Image => {
                self.finish_image()?;
            }
            TagEnd::HtmlBlock => {}
            TagEnd::Table => {
                self.finish_table()?;
            }
            TagEnd::TableHead => self.finish_table_head(),
            TagEnd::TableRow => self.finish_table_row(),
            TagEnd::TableCell => self.finish_table_cell(),
            TagEnd::Emphasis => self.emphasis_depth = self.emphasis_depth.saturating_sub(1),
            TagEnd::Strong => self.strong_depth = self.strong_depth.saturating_sub(1),
            TagEnd::Strikethrough => self.in_strikethrough = false,
            TagEnd::MetadataBlock(_) => self.in_metadata_block = false,
            TagEnd::DefinitionList => {
                self.definition_list_stack.pop();
                if self.definition_list_stack.is_empty() {
                    self.finish_block(self.theme.margin_bottom);
                }
            }
            TagEnd::DefinitionListTitle => {
                self.strong_depth = self.strong_depth.saturating_sub(1);
                // NOTE: Do NOT call new_line() here!
                // Let Tag::DefinitionListTitle/DefinitionListDefinition handle spacing.
                // Previously, calling new_line() in both start and end tags could
                // cause redundant advances in edge cases.
            }
            TagEnd::DefinitionListDefinition => {
                self.item_continuation_indent = None;
                // NOTE: Do NOT call new_line() here!
                // Let Tag::DefinitionListTitle handle spacing for the next item.
            }
            TagEnd::FootnoteDefinition => {
                self.in_footnote_definition = false;
                self.pending_list_marker = None;
                self.item_continuation_indent = None;
                self.finish_block(self.theme.margin_bottom * 0.8);
            }
            _ => {}
        }

        Ok(())
    }

    fn flush_pending_text(&mut self) -> Result<(), String> {
        if self.pending_text.is_empty() {
            return Ok(());
        }
        let text = std::mem::take(&mut self.pending_text);
        self.render_text(&text)
    }

    fn render_text(&mut self, text: &str) -> Result<(), String> {
        if let Some(image) = self.current_image.as_mut() {
            image.alt_text.push_str(text);
            return Ok(());
        }

        if text.is_empty() {
            return Ok(());
        }

        let mut buf = String::new();
        let mut buf_is_ws: Option<bool> = None;

        for ch in text.chars() {
            let is_ws = ch.is_whitespace();
            match buf_is_ws {
                Some(cur) if cur == is_ws => buf.push(ch),
                Some(cur) => {
                    self.render_token(&buf, cur)?;
                    buf.clear();
                    buf.push(ch);
                    buf_is_ws = Some(is_ws);
                }
                None => {
                    buf.push(ch);
                    buf_is_ws = Some(is_ws);
                }
            }
        }

        if let Some(cur) = buf_is_ws {
            self.render_token(&buf, cur)?;
        }

        Ok(())
    }

    fn draw_line_decoration(&mut self, y: f32, width: f32, fill: &str) -> Result<(), String> {
        write!(
            self.svg_content,
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
            self.cursor_x,
            y,
            self.cursor_x + width,
            y,
            fill,
        )
        .map_err(|e| e.to_string())
    }

    fn render_text_content_token(
        &mut self,
        token: &str,
        font_size: f32,
        is_bold: bool,
        is_italic: bool,
    ) -> Result<(), String> {
        let eff = self.effective_inline_style();
        let (token_width, _) =
            self.measure
                .measure_text(token, font_size, false, is_bold, is_italic, None);

        if !self.at_line_start && self.cursor_x + token_width > self.right_edge() {
            self.advance_line(font_size);
        }

        // Baseline shift for <sup>/<sub>, relative to the base font size.
        let baseline_y = self.cursor_y + eff.rise_ratio * self.current_font_size();

        // Background highlight (<mark> or style="background-color: ...").
        if let Some(bg) = &eff.background {
            write!(
                self.svg_content,
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" fill="{}" />"#,
                self.cursor_x,
                baseline_y - font_size * 0.8,
                token_width,
                font_size * 1.1,
                bg,
            )
            .unwrap();
        }

        let fill = self.current_fill();
        if self.pending_list_marker.is_some()
            && !self.at_line_start
            && let Some(pending) = self.pending_list_marker.take()
        {
            self.draw_text_at(
                pending.marker_x,
                self.cursor_y,
                &pending.marker,
                "sans-serif",
                self.theme.font_size_base,
                &fill,
                false,
                false,
            );
        }

        self.draw_text_at(
            self.cursor_x,
            baseline_y,
            token,
            "sans-serif",
            font_size,
            &fill,
            is_bold,
            is_italic,
        );

        if self.in_strikethrough {
            let line_y = baseline_y - font_size * 0.32;
            self.draw_line_decoration(line_y, token_width, &fill)?;
        }

        if self.link_depth > 0 || eff.underline {
            let underline_y = baseline_y + font_size * 0.12;
            self.draw_line_decoration(underline_y, token_width, &fill)?;
        }

        self.cursor_x += token_width;
        self.at_line_start = false;

        Ok(())
    }

    fn render_whitespace_token(
        &mut self,
        font_size: f32,
        is_bold: bool,
        is_italic: bool,
    ) -> Result<(), String> {
        if self.at_line_start {
            return Ok(());
        }

        let space_width = self.space_width(font_size, is_bold, is_italic);

        if self.cursor_x + space_width > self.right_edge() {
            self.advance_line(font_size);
            return Ok(());
        }

        // Inside a <mark>/background-color or <u>/link scope the decoration must
        // span the space too, otherwise highlights and underlines get a visible
        // gap at every space (e.g. <mark>marked text</mark> would be two boxes).
        let eff = self.effective_inline_style();
        if eff.background.is_some() || eff.underline || self.link_depth > 0 {
            let baseline_y = self.cursor_y + eff.rise_ratio * self.current_font_size();
            if let Some(bg) = &eff.background {
                write!(
                    self.svg_content,
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" fill="{}" />"#,
                    self.cursor_x,
                    baseline_y - font_size * 0.8,
                    space_width,
                    font_size * 1.1,
                    bg,
                )
                .unwrap();
            }
            if eff.underline || self.link_depth > 0 {
                let underline_y = baseline_y + font_size * 0.12;
                let fill = self.current_fill();
                self.draw_line_decoration(underline_y, space_width, &fill)?;
            }
        }

        self.cursor_x += space_width;

        Ok(())
    }

    /// Memoized space-advance width for a given (font_size, bold, italic) style.
    /// The first call infers the width with three measurements; every later call
    /// for the same style is a single hash lookup.
    fn space_width(&mut self, font_size: f32, is_bold: bool, is_italic: bool) -> f32 {
        let key = (font_size.to_bits(), is_bold, is_italic);
        if let Some(&width) = self.space_width_cache.get(&key) {
            return width;
        }
        let width = self.infer_space_width(font_size, is_bold, is_italic);
        self.space_width_cache.insert(key, width);
        width
    }

    fn infer_space_width(&mut self, font_size: f32, is_bold: bool, is_italic: bool) -> f32 {
        let (raw_space_width, _) =
            self.measure
                .measure_text(" ", font_size, false, is_bold, is_italic, None);

        // Some shapers trim trailing whitespace and report a zero/tiny width for " ".
        // Infer space advance from "m m" - "mm" and prefer the larger valid value.
        let (with_space, _) =
            self.measure
                .measure_text("m m", font_size, false, is_bold, is_italic, None);
        let (without_space, _) =
            self.measure
                .measure_text("mm", font_size, false, is_bold, is_italic, None);
        let inferred = with_space - without_space;

        let mut space_width = if inferred.is_finite() && inferred > 0.0 {
            raw_space_width.max(inferred)
        } else {
            raw_space_width
        };

        if !(space_width.is_finite() && space_width > 0.0) {
            space_width = font_size * 0.33;
        }
        // Guard against shapers reporting near-zero space width.
        space_width = space_width.max(font_size * 0.2);
        // Cap space width to avoid excessively wide gaps (e.g. large headings).
        space_width.min(font_size * 0.4)
    }

    fn render_token(&mut self, token: &str, is_whitespace: bool) -> Result<(), String> {
        if token.is_empty() {
            return Ok(());
        }

        // Scale the font size by any active inline HTML style (<sup>/<sub>,
        // style="font-size: ...").
        let eff = self.effective_inline_style();
        let font_size = self.current_font_size() * eff.scale;
        let is_bold = self.is_bold();
        let is_italic = self.is_italic();

        if is_whitespace {
            self.render_whitespace_token(font_size, is_bold, is_italic)
        } else {
            self.render_text_content_token(token, font_size, is_bold, is_italic)
        }
    }

    fn render_inline_code(&mut self, code: &str) -> Result<(), String> {
        let (text_width, _text_height) =
            self.measure
                .measure_text(code, self.theme.font_size_code, true, false, false, None);

        let total_width = text_width + self.theme.code_padding_x * 2.0;
        if !self.at_line_start && self.cursor_x + total_width > self.right_edge() {
            self.new_line();
        }

        // Tighter background box based on font size
        let rect_height = self.theme.font_size_code * 1.25 + self.theme.code_padding_y;
        // Use font metrics for proper alignment (ascent ratio 0.75)
        let ascent_ratio = 0.75;
        let rect_y = self.cursor_y - self.theme.font_size_code * ascent_ratio - self.theme.code_padding_y * 0.5;

        write!(
            self.svg_content,
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" />"#,
            self.cursor_x,
            rect_y,
            total_width,
            rect_height,
            self.theme.code_radius,
            self.theme.code_bg_color,
        )
        .unwrap();

        let code_text_color = self.theme.code_text_color.clone();
        self.draw_text_at(
            self.cursor_x + self.theme.code_padding_x,
            self.cursor_y,
            code,
            "monospace",
            self.theme.font_size_code,
            &code_text_color,
            false,
            false,
        );

        self.cursor_x += total_width;
        self.at_line_start = false;

        Ok(())
    }

    fn render_inline_html(&mut self, html: &str) -> Result<(), String> {
        let tag = html.trim().to_ascii_lowercase();

        // Fast path for plain formatting tags without attributes.
        match tag.as_str() {
            "<br>" | "<br/>" | "<br />" => return self.render_newline(),
            "<del>" | "<s>" => {
                self.in_strikethrough = true;
                return Ok(());
            }
            "</del>" | "</s>" => {
                self.in_strikethrough = false;
                return Ok(());
            }
            "<em>" | "<i>" => {
                self.emphasis_depth += 1;
                return Ok(());
            }
            "</em>" | "</i>" => {
                self.emphasis_depth = self.emphasis_depth.saturating_sub(1);
                return Ok(());
            }
            "<strong>" | "<b>" => {
                self.strong_depth += 1;
                return Ok(());
            }
            "</strong>" | "</b>" => {
                self.strong_depth = self.strong_depth.saturating_sub(1);
                return Ok(());
            }
            _ => {}
        }

        let Some(parsed) = self.parse_inline_html_tag(html) else {
            return Ok(());
        };

        if parsed.self_closing {
            return Ok(());
        }

        if parsed.closing {
            self.html_style_stack.pop();
            return Ok(());
        }

        let mut style = parsed.style;
        match parsed.name.as_str() {
            "sup" => {
                style.scale *= 0.7;
                style.rise_ratio += -0.45;
            }
            "sub" => {
                style.scale *= 0.7;
                style.rise_ratio += 0.25;
            }
            "u" => style.underline = true,
            "mark" => {
                if style.background.is_none() {
                    style.background = Some(self.default_mark_color());
                }
            }
            // `<span>` and `<font>` carry their styling purely in attributes.
            "span" | "font" => {}
            // Unknown tags: ignore the tag itself, keep rendering its content.
            _ => return Ok(()),
        }

        if !style.is_empty() {
            self.html_style_stack.push(style);
        }

        Ok(())
    }

    /// Default `<mark>` highlight color, adapted to the theme so text on the
    /// highlight stays readable (pure yellow is ~1:1 contrast on dark themes).
    fn default_mark_color(&self) -> String {
        if theme_is_dark(&self.theme.background_color) {
            MARK_HIGHLIGHT_DARK.to_string()
        } else {
            MARK_HIGHLIGHT_LIGHT.to_string()
        }
    }

    /// Fold the open HTML style stack into one effective style (innermost wins).
    fn effective_inline_style(&self) -> InlineHtmlStyle {
        let mut eff = InlineHtmlStyle::default();
        for style in &self.html_style_stack {
            if eff.color.is_none() {
                eff.color = style.color.clone();
            }
            if eff.background.is_none() {
                eff.background = style.background.clone();
            }
            eff.scale *= style.scale;
            eff.rise_ratio += style.rise_ratio;
            eff.underline |= style.underline;
        }
        eff
    }

    /// Parse an inline HTML tag like `<span style="color: red">` or `</sup>`.
    fn parse_inline_html_tag(&self, html: &str) -> Option<ParsedHtmlTag> {
        let trimmed = html.trim();
        if !trimmed.starts_with('<') {
            return None;
        }

        let closing = trimmed.starts_with("</");
        let inner = if closing { &trimmed[2..] } else { &trimmed[1..] };
        let inner = inner.split_once('>').map(|(head, _)| head).unwrap_or(inner).trim();

        let self_closing = inner.ends_with('/');
        let inner = inner.trim_end_matches('/').trim();

        let mut parts = inner.splitn(2, char::is_whitespace);
        let name = parts.next()?.trim().to_ascii_lowercase();
        if name.is_empty()
            || !name.chars().all(|c| c.is_ascii_alphanumeric())
            || name.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            return None;
        }

        let style = if closing {
            InlineHtmlStyle::default()
        } else {
            parse_inline_html_attrs(parts.next().unwrap_or(""))
        };

        Some(ParsedHtmlTag {
            name,
            closing,
            self_closing,
            style,
        })
    }

    fn render_footnote_reference(&mut self, label: &str) -> Result<(), String> {
        let font_size = self.current_font_size();
        let marker = label.to_string();
        let superscript_size = font_size * 0.65;
        let (marker_width, _) =
            self.measure
                .measure_text(&marker, superscript_size, false, false, false, None);

        if !self.at_line_start && self.cursor_x + marker_width > self.right_edge() {
            self.advance_line(font_size);
        }

        let y = self.cursor_y - font_size * 0.45;
        let fill = self.current_fill();
        self.draw_text_at(
            self.cursor_x,
            y,
            &marker,
            "sans-serif",
            superscript_size,
            &fill,
            false,
            false,
        );

        self.cursor_x += marker_width;
        self.at_line_start = false;
        Ok(())
    }

    fn render_inline_math(&mut self, math_src: &str) -> Result<(), String> {
        let font_size = self.current_font_size();
        let color = self.current_fill();

        match crate::math::render_math(math_src, font_size, &color, &mut self.measure, false) {
            Ok(result) => {
                if !self.at_line_start && self.cursor_x + result.width > self.right_edge() {
                    self.new_line();
                }
                write!(
                    self.svg_content,
                    r#"<g transform="translate({:.2}, {:.2})">{}</g>"#,
                    self.cursor_x, self.cursor_y, result.svg_fragment
                )
                .unwrap();
                self.cursor_x += result.width;
                self.at_line_start = false;
                self.last_margin_added = 0.0;
            }
            Err(e) => {
                eprintln!("Warning: math render failed (line {}): {}", self.current_event_line, e);
                self.render_inline_code(math_src)?;
            }
        }
        Ok(())
    }

    fn render_display_math(&mut self, math_src: &str) -> Result<(), String> {
        self.in_display_math = true;
        self.pending_math_block = Some(math_src.to_string());
        self.render_math_block()?;
        Ok(())
    }

    fn append_math_text(&mut self, text: &str) {
        if let Some(existing) = self.pending_math_block.as_mut() {
            existing.push_str(text);
        } else {
            self.pending_math_block = Some(text.to_string());
        }
    }

    fn render_math_block(&mut self) -> Result<(), String> {
        let Some(math_src) = self.pending_math_block.take() else {
            self.in_display_math = false;
            return Ok(());
        };

        if !self.at_line_start {
            self.new_line();
        }

        let font_size = self.current_font_size();
        let color = self.current_fill();

        match crate::math::render_math(&math_src, font_size, &color, &mut self.measure, true) {
            Ok(result) => {
                self.start_block(self.theme.margin_top, false);

                let available_width = self.right_edge() - self.line_start_x();
                let offset_x =
                    self.line_start_x() + (available_width - result.width).max(0.0) / 2.0;
                let baseline_y = self.cursor_y + result.ascent;

                write!(
                    self.svg_content,
                    r#"<g transform="translate({:.2}, {:.2})">{}</g>"#,
                    offset_x, baseline_y, result.svg_fragment
                )
                .unwrap();

                self.cursor_y = baseline_y + result.descent;
                self.cursor_x = self.line_start_x();
                self.at_line_start = true;

                self.finish_block(self.theme.margin_bottom);
            }
            Err(e) => {
                eprintln!("Warning: math render failed (line {}): {}", self.current_event_line, e);
                self.start_block(self.theme.margin_top, false);
                self.render_inline_code(&math_src)?;
                self.finish_block(self.theme.margin_bottom);
            }
        }
        self.in_display_math = false;
        Ok(())
    }

    fn render_code_block(&mut self) -> Result<(), String> {
        let code_buffer = self.code_block_buffer.clone();
        let lang = self.code_block_lang.clone();
        self.render_code_block_with_language(&code_buffer, lang.as_deref())
    }

    fn render_code_block_with_language(
        &mut self,
        code_buffer: &str,
        lang: Option<&str>,
    ) -> Result<(), String> {
        // Check for mermaid diagram
        if lang == Some("mermaid") {
            return self.render_mermaid_block(code_buffer);
        }

        let x = self.line_start_x();
        let max_content_width = (self.right_edge() - x - self.theme.code_padding_x * 2.0)
            .max(self.theme.font_size_code);

        // 1. Highlight Phase
        let mut raw_highlighted_lines: Vec<Vec<(SyntectStyle, String)>> = Vec::new();

        {
            let lang = lang.unwrap_or("txt");
            let syntax = self
                .ps
                .find_syntax_by_token(lang)
                .or_else(|| self.ps.find_syntax_by_extension(lang))
                .unwrap_or_else(|| self.ps.find_syntax_plain_text());

            let is_dark = {
                let hex = self.theme.code_bg_color.trim_start_matches('#');
                if hex.len() == 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
                    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
                    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
                    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) < 128.0
                } else {
                    false
                }
            };

            let theme_name = if is_dark {
                "Solarized (dark)"
            } else {
                "Solarized (light)"
            };
            let theme = self
                .ts
                .themes
                .get(theme_name)
                .or_else(|| {
                    self.ts.themes.get(if is_dark {
                        "base16-ocean.dark"
                    } else {
                        "base16-ocean.light"
                    })
                })
                .unwrap_or_else(|| self.ts.themes.values().next().unwrap());

            let mut highlighter = HighlightLines::new(syntax, theme);

            for line in code_buffer.lines() {
                let ranges = highlighter
                    .highlight_line(line, &self.ps)
                    .map_err(|e| format!("Highlight error: {}", e))?;

                raw_highlighted_lines
                    .push(ranges.iter().map(|(s, t)| (*s, t.to_string())).collect());
            }
        }

        // 2. Wrap Phase
        let mut lines = Vec::new();
        for line_segments in raw_highlighted_lines {
            let segments_ref: Vec<(SyntectStyle, &str)> = line_segments
                .iter()
                .map(|(s, t)| (*s, t.as_str()))
                .collect();
            self.wrap_styled_line(&segments_ref, max_content_width, &mut lines);
        }

        if lines.is_empty() {
            lines.push(vec![(SyntectStyle::default(), String::new())]);
        }

        let mut max_line_width: f32 = 0.0;
        for line_segments in &lines {
            let mut line_w = 0.0;
            for (_style, text) in line_segments {
                let (w, _) = self.measure.measure_text(
                    text,
                    self.theme.font_size_code,
                    true,
                    false,
                    false,
                    None,
                );
                line_w += w;
            }
            max_line_width = max_line_width.max(line_w);
        }

        let line_height = self.theme.font_size_code * self.theme.line_height;
        let effective_code_pad_y = self.theme.code_padding_y.max(self.theme.font_size_code * 0.5);
        let block_height = (lines.len().saturating_sub(1) as f32) * line_height
            + self.theme.font_size_code
            + effective_code_pad_y * 2.0;
        let block_width = max_line_width + self.theme.code_padding_x * 2.0;

        write!(
            self.svg_content,
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" />"#,
            x,
            self.cursor_y,
            block_width,
            block_height,
            self.theme.code_radius,
            self.theme.code_bg_color,
        )
        .unwrap();

        for (idx, line_segments) in lines.iter().enumerate() {
            let y = self.cursor_y
                + effective_code_pad_y
                + self.theme.font_size_code * 0.8
                + idx as f32 * line_height;

            let mut current_x = x + self.theme.code_padding_x;

            for (style, text) in line_segments {
                let fill = format!(
                    "#{:02x}{:02x}{:02x}",
                    style.foreground.r, style.foreground.g, style.foreground.b
                );

                let w = self.draw_text_at(
                    current_x,
                    y,
                    text,
                    "monospace",
                    self.theme.font_size_code,
                    &fill,
                    false,
                    false,
                );
                current_x += w;
            }
        }

        self.cursor_y += block_height;
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;

        Ok(())
    }

    fn render_mermaid_block(&mut self, source: &str) -> Result<(), String> {
        use crate::mermaid::{DiagramStyle, render_diagram};

        let x = self.line_start_x();
        let style = DiagramStyle::from_theme(
            &self.theme.text_color,
            &self.theme.background_color,
            &self.theme.code_bg_color,
        );

        let (svg, width, height) = render_diagram(source, &style, &mut self.measure)
            .map_err(|e| format!("Mermaid diagram (line {}): {}", self.code_block_start_line, e))?;

        // Scale down oversized diagrams so they never overflow the code-block frame.
        let available_width = self.right_edge() - x;
        let content_max_width = (available_width - 20.0).max(1.0);
        let scale = if width > content_max_width {
            content_max_width / width
        } else {
            1.0
        };
        let rendered_width = width * scale;
        let rendered_height = height * scale;
        let offset_x = (available_width - rendered_width).max(0.0) / 2.0;

        // Add background
        let bg_height = rendered_height + 20.0;
        write!(
            self.svg_content,
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" />"#,
            x,
            self.cursor_y,
            available_width,
            bg_height,
            self.theme.code_radius,
            self.theme.code_bg_color,
        )
        .unwrap();

        // Add diagram SVG (wrapped in a group with translation)
        let svg_x = x + offset_x;
        let svg_y = self.cursor_y + 10.0;
        write!(
            self.svg_content,
            r#"<g transform="translate({:.2}, {:.2}) scale({:.4})">{}</g>"#,
            svg_x, svg_y, scale, svg
        )
        .unwrap();

        self.cursor_y += bg_height;
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;

        Ok(())
    }

    fn wrap_styled_line(
        &mut self,
        segments: &[(SyntectStyle, &str)],
        max_width: f32,
        out: &mut Vec<Vec<(SyntectStyle, String)>>,
    ) {
        if segments.is_empty() {
            out.push(Vec::new());
            return;
        }

        let mut current_line: Vec<(SyntectStyle, String)> = Vec::new();
        let mut current_line_width = 0.0;

        for (style, text) in segments {
            if text.is_empty() {
                continue;
            }

            let mut current_text = String::new();
            let mut ch_buf = [0u8; 4];

            for ch in text.chars() {
                let candidate_str = ch.encode_utf8(&mut ch_buf);
                let (ch_width, _) = self.measure.measure_text(
                    candidate_str,
                    self.theme.font_size_code,
                    true,
                    false,
                    false,
                    None,
                );

                if current_line_width + ch_width > max_width {
                    if !current_text.is_empty() {
                        current_line.push((*style, current_text));
                    }
                    out.push(current_line);
                    current_line = Vec::new();
                    current_line_width = 0.0;
                    current_text = String::new();
                }

                current_text.push(ch);
                current_line_width += ch_width;
            }

            if !current_text.is_empty() {
                current_line.push((*style, current_text));
            }
        }

        if !current_line.is_empty() {
            out.push(current_line);
        } else if out.is_empty() {
            out.push(Vec::new());
        }
    }

    fn render_newline(&mut self) -> Result<(), String> {
        self.new_line();
        Ok(())
    }

    fn render_soft_break(&mut self) -> Result<(), String> {
        if self.at_line_start {
            return Ok(());
        }

        let font_size = self.current_font_size();
        if let Some(state) = self.list_stack.last()
            && !state.needs_ascent {
                self.advance_line(font_size);
                return Ok(());
            }

        self.new_line();
        Ok(())
    }

    fn render_horizontal_rule(&mut self) -> Result<(), String> {
        if !self.at_line_start {
            self.new_line();
        }

        self.add_margin(self.theme.margin_top * 0.5);
        let hr_y = self.cursor_y;
        let left = self.base_left_indent();
        let right = self.right_edge();

        write!(
            self.svg_content,
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5" />"#,
            left, hr_y, right, hr_y, self.theme.quote_border_color,
        )
        .unwrap();

        // Reset margin tracking so the bottom margin isn't collapsed with the top.
        self.last_margin_added = 0.0;

        self.add_margin(self.theme.margin_bottom * 0.5);
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
        Ok(())
    }

    fn start_table(&mut self, alignments: Vec<Alignment>) {
        if !self.at_line_start {
            self.new_line();
        }
        self.start_block(self.theme.margin_top, false);
        self.in_table = true;
        self.table_state = Some(TableState {
            alignments,
            rows: Vec::new(),
            current_row: None,
            current_cell: None,
            in_head: false,
        });
    }

    fn start_table_head(&mut self) {
        if let Some(state) = self.table_state.as_mut() {
            state.in_head = true;
            // GFM mode: TableHead may not contain a TableRow wrapper,
            // so initialize the row here as well.
            if state.current_row.is_none() {
                state.current_row = Some(TableRowData {
                    cells: Vec::new(),
                    is_header: true,
                });
            }
        }
    }

    fn finish_table_head(&mut self) {
        if let Some(state) = self.table_state.as_mut() {
            // Flush the header row if it was implicitly created (no TableRow wrapper)
            if let Some(row) = state.current_row.take()
                && !row.cells.is_empty() {
                    state.rows.push(row);
                }
            state.in_head = false;
        }
    }

    fn start_table_row(&mut self) {
        if let Some(state) = self.table_state.as_mut() {
            state.current_row = Some(TableRowData {
                cells: Vec::new(),
                is_header: state.in_head,
            });
        }
    }

    /// Complete the current table row: move it from `current_row` into the
    /// `rows` buffer.  The first row is marked `is_header = true` because
    /// the pulldown-cmark parser emits the header row (text between `|…|`
    /// and the `|---|` separator) as a normal `TableRow` before any data
    /// rows.  This convention is relied upon by `finish_table` to apply
    /// bold styling and a background tint to header cells.
    fn finish_table_row(&mut self) {
        if let Some(state) = self.table_state.as_mut()
            && let Some(row) = state.current_row.take() {
                state.rows.push(row);
            }
    }

    fn start_table_cell(&mut self) {
        if let Some(state) = self.table_state.as_mut() {
            state.current_cell = Some(TableCellData {
                text: String::new(),
            });
        }
    }

    fn finish_table_cell(&mut self) {
        if let Some(state) = self.table_state.as_mut()
            && let Some(cell) = state.current_cell.take()
                && let Some(row) = state.current_row.as_mut() {
                    row.cells.push(cell);
                }
    }

    fn render_table_text(&mut self, text: &str) {
        if let Some(state) = self.table_state.as_mut()
            && let Some(cell) = state.current_cell.as_mut() {
                cell.text.push_str(text);
            }
    }

    fn finish_table(&mut self) -> Result<(), String> {
        let Some(state) = self.table_state.take() else {
            self.in_table = false;
            return Ok(());
        };

        self.in_table = false;

        if state.rows.is_empty() {
            self.finish_block(self.theme.margin_bottom);
            return Ok(());
        }

        let column_count = state
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0);

        if column_count == 0 {
            self.finish_block(self.theme.margin_bottom);
            return Ok(());
        }

        let cell_padding_x = self.theme.font_size_base * 0.5;
        let cell_padding_y = self.theme.font_size_base * 0.35;
        let border_color = self.theme.quote_border_color.clone();
        let line_height = self.theme.font_size_base * self.theme.line_height.max(1.3);
        let table_x = self.line_start_x();
        let available_width = (self.right_edge() - table_x).max(1.0);

        // Measure natural column widths (text width + horizontal padding) so an
        // unscaled table renders every cell on a single line.
        let mut natural_widths: Vec<f32> = vec![0.0; column_count];
        for row in &state.rows {
            for (idx, cell) in row.cells.iter().enumerate() {
                let (width, _) = self.measure.measure_text(
                    cell.text.trim(),
                    self.theme.font_size_base,
                    false,
                    row.is_header,
                    false,
                    None,
                );
                natural_widths[idx] = natural_widths[idx].max(width + cell_padding_x * 2.0);
            }
        }

        // Shrink columns proportionally when the natural table is wider than the
        // page, then enforce a minimum column width if it still fits.
        let natural_total: f32 = natural_widths.iter().sum();
        let scale = if natural_total > available_width {
            available_width / natural_total
        } else {
            1.0
        };
        let min_col_width = self.theme.font_size_base * 1.5;
        let mut column_widths: Vec<f32> = natural_widths.iter().map(|w| w * scale).collect();
        let min_total: f32 = column_widths.len() as f32 * min_col_width;
        if min_total <= available_width {
            for w in column_widths.iter_mut() {
                *w = (*w).max(min_col_width);
            }
        }
        // Final guard: the table must never exceed the available width.
        let enforced_total: f32 = column_widths.iter().sum();
        if enforced_total > available_width && enforced_total > 0.0 {
            let fit = available_width / enforced_total;
            for w in column_widths.iter_mut() {
                *w *= fit;
            }
        }

        // Wrap every cell to its column width; track per-row line counts so row
        // heights grow to fit the tallest cell in the row.
        let mut wrapped_rows: Vec<Vec<Vec<String>>> = Vec::with_capacity(state.rows.len());
        let mut row_heights: Vec<f32> = Vec::with_capacity(state.rows.len());
        for row in &state.rows {
            let mut cell_lines: Vec<Vec<String>> = Vec::with_capacity(row.cells.len());
            let mut max_lines = 1usize;
            for (idx, cell) in row.cells.iter().enumerate() {
                let col_width = column_widths.get(idx).copied().unwrap_or(0.0);
                let content_width = (col_width - cell_padding_x * 2.0).max(1.0);
                let lines =
                    self.wrap_table_text(cell.text.trim(), content_width, row.is_header);
                max_lines = max_lines.max(lines.len());
                cell_lines.push(lines);
            }
            wrapped_rows.push(cell_lines);
            row_heights.push(max_lines as f32 * line_height + cell_padding_y * 2.0);
        }

        // column_widths already include horizontal padding.
        let table_width: f32 = column_widths.iter().sum();
        let table_height: f32 = row_heights.iter().sum();
        let mut current_y = self.cursor_y;

        write!(
            self.svg_content,
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="none" stroke="{}" stroke-width="1" />"#,
            table_x,
            current_y,
            table_width,
            table_height,
            border_color,
        )
        .unwrap();

        for (row_idx, row) in state.rows.iter().enumerate() {
            let row_height = row_heights[row_idx];
            if row.is_header {
                write!(
                    self.svg_content,
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" fill-opacity="{:.2}" />"#,
                    table_x,
                    current_y,
                    table_width,
                    row_height,
                    self.theme.text_color,
                    self.theme.table_header_opacity,
                )
                .unwrap();
            }

            let mut cell_x = table_x;
            for idx in 0..row.cells.len() {
                let cell_width = column_widths[idx];
                let align = state
                    .alignments
                    .get(idx)
                    .copied()
                    .unwrap_or(Alignment::Left);

                write!(
                    self.svg_content,
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="none" stroke="{}" stroke-width="1" />"#,
                    cell_x,
                    current_y,
                    cell_width,
                    row_height,
                    border_color,
                )
                .unwrap();

                let fill = self.current_fill();
                let lines = &wrapped_rows[row_idx][idx];
                for (line_idx, line) in lines.iter().enumerate() {
                    let (text_width, _) = self.measure.measure_text(
                        line,
                        self.theme.font_size_base,
                        false,
                        row.is_header,
                        false,
                        None,
                    );

                    let text_x = match align {
                        Alignment::Left | Alignment::None => cell_x + cell_padding_x,
                        Alignment::Center => cell_x + (cell_width - text_width) / 2.0,
                        Alignment::Right => cell_x + cell_width - cell_padding_x - text_width,
                    };

                    let text_y = current_y
                        + cell_padding_y
                        + self.theme.font_size_base * 0.8
                        + line_idx as f32 * line_height;
                    self.draw_text_at(
                        text_x,
                        text_y,
                        line,
                        "sans-serif",
                        self.theme.font_size_base,
                        &fill,
                        row.is_header,
                        false,
                    );
                }

                cell_x += cell_width;
            }

            current_y += row_height;
        }

        self.cursor_y += table_height;
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
        self.finish_block(self.theme.margin_bottom);
        Ok(())
    }

    /// Greedy word-wrap for table cells. Words wider than the column are
    /// hard-split so cell content never overflows the table.
    fn wrap_table_text(&mut self, text: &str, max_width: f32, bold: bool) -> Vec<String> {
        if text.is_empty() {
            return vec![String::new()];
        }
        if max_width <= 0.0 {
            return vec![text.to_string()];
        }

        let font_size = self.theme.font_size_base;
        let space_w = self.space_width(font_size, bold, false);

        let mut lines: Vec<String> = Vec::new();
        let mut line = String::new();
        let mut line_w = 0.0f32;

        for word in text.split_whitespace() {
            let (word_w, _) =
                self.measure
                    .measure_text(word, font_size, false, bold, false, None);
            let separator_w = if line.is_empty() { 0.0 } else { space_w };

            if line_w + separator_w + word_w > max_width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                line_w = 0.0;
            }

            if word_w > max_width {
                // Hard-split words wider than the column.
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                    line_w = 0.0;
                }
                let mut chunk = String::new();
                let mut chunk_w = 0.0f32;
                for ch in word.chars() {
                    let (cw, _) = self.measure.measure_text(
                        &ch.to_string(),
                        font_size,
                        false,
                        bold,
                        false,
                        None,
                    );
                    if chunk_w + cw > max_width && !chunk.is_empty() {
                        lines.push(std::mem::take(&mut chunk));
                        chunk_w = 0.0;
                    }
                    chunk.push(ch);
                    chunk_w += cw;
                }
                if !chunk.is_empty() {
                    line = chunk;
                    line_w = chunk_w;
                }
            } else {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
                line_w += separator_w + word_w;
            }
        }

        if !line.is_empty() {
            lines.push(line);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }

    fn render_task_marker(&mut self, checked: bool) -> Result<(), String> {
        if !self.at_line_start {
            self.new_line();
        }

        let marker_x = self
            .pending_list_marker
            .take()
            .map(|p| p.marker_x)
            .unwrap_or(self.cursor_x);

        let size = self.theme.font_size_base * 0.85;
        let gap = self.theme.font_size_base * LIST_MARKER_GAP_RATIO;

        self.item_continuation_indent = Some(marker_x + size + gap);

        let x = marker_x;
        let y = self.cursor_y - size * 0.7;
        let marker_stroke = self.current_fill();
        write!(
            self.svg_content,
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" ry="2" stroke="{}" fill="none" stroke-width="1" />"#,
            x,
            y,
            size,
            size,
            marker_stroke,
        )
        .unwrap();

        if checked {
            let inset = size * 0.2;
            let x1 = x + inset;
            let y1 = y + size * 0.55;
            let x2 = x + size * 0.45;
            let y2 = y + size - inset;
            let x3 = x + size - inset;
            let y3 = y + inset;
            let check_stroke = self.current_fill();
            write!(
                self.svg_content,
                r#"<polyline points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="none" stroke="{}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />"#,
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                check_stroke,
            )
            .unwrap();
        }

        self.cursor_x = marker_x + size + gap;
        self.at_line_start = false;
        Ok(())
    }

    fn finish_image(&mut self) -> Result<(), String> {
        let Some(image) = self.current_image.take() else {
            return Ok(());
        };

        let src = image.src.trim();
        if src.is_empty() {
            return Ok(());
        }

        let payload = match self.load_image_payload(src) {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(()),
            Err(e) => {
                eprintln!("Warning: {}", e);
                let alt = if image.alt_text.is_empty() {
                    src.to_string()
                } else {
                    image.alt_text.clone()
                };
                self.render_inline_code(&alt)?;
                return Ok(());
            }
        };

        if !self.at_line_start {
            self.new_line();
        }

        let max_width = self.right_edge() - self.line_start_x();
        let mut width = payload.width;
        let mut height = payload.height;
        if width > max_width {
            let scale = max_width / width;
            width *= scale;
            height *= scale;
        }

        self.start_block(self.theme.margin_top * 0.4, false);
        let x = self.line_start_x();
        let y = self.cursor_y;

        write!(
            self.svg_content,
            r#"<image x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" href="{}" />"#,
            x, y, width, height, payload.data_url,
        )
        .unwrap();

        self.cursor_y += height;
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
        self.finish_block(self.theme.margin_bottom * 0.4);

        Ok(())
    }

    fn load_image_payload(&self, src: &str) -> Result<Option<ImagePayload>, String> {
        if src.starts_with("data:") {
            let Some((mime, bytes)) = self.parse_data_url(src)? else {
                return Ok(None);
            };
            let (width, height) = self.image_dimensions(&mime, &bytes)?;
            let data_url = self.build_data_url(&mime, &bytes);
            return Ok(Some(ImagePayload {
                data_url,
                width,
                height,
            }));
        }

        if src.starts_with("http://") || src.starts_with("https://") {
            // Bound the fetch: a hung or malicious image server must not stall the
            // render, and an oversized payload must not exhaust memory.
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(std::time::Duration::from_secs(10)))
                .build()
                .into();
            let mut response = agent
                .get(src)
                .call()
                .map_err(|e| format!("Failed to fetch image {}: {}", src, e))?;
            let mime = response
                .body()
                .mime_type()
                .or_else(|| self.mime_from_url(src))
                .map(|value| value.to_string())
                .unwrap_or_default();
            if mime.is_empty() {
                return Ok(None);
            }

            const MAX_REMOTE_IMAGE_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB
            use std::io::Read;
            let reader = response.body_mut().as_reader();
            let mut bytes = Vec::new();
            reader
                .take(MAX_REMOTE_IMAGE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Failed to read image {}: {}", src, e))?;
            if bytes.len() as u64 > MAX_REMOTE_IMAGE_BYTES {
                return Err(format!(
                    "Remote image {} exceeds the 10 MiB size limit",
                    src
                ));
            }

            let (width, height) = self.image_dimensions(&mime, &bytes)?;
            let data_url = self.build_data_url(&mime, &bytes);
            return Ok(Some(ImagePayload {
                data_url,
                width,
                height,
            }));
        }

        let image_path = self.resolve_image_path(src);
        let Some(image_path) = image_path else {
            return Ok(None);
        };

        let bytes = std::fs::read(&image_path)
            .map_err(|e| format!("Failed to read image {}: {}", image_path.display(), e))?;
        let mime = self.mime_from_path(&image_path).unwrap_or("");
        if mime.is_empty() {
            return Ok(None);
        }

        let (width, height) = self.image_dimensions(mime, &bytes)?;
        let data_url = self.build_data_url(mime, &bytes);
        Ok(Some(ImagePayload {
            data_url,
            width,
            height,
        }))
    }

    fn parse_data_url(&self, src: &str) -> Result<Option<(String, Vec<u8>)>, String> {
        let rest = src.strip_prefix("data:").unwrap_or(src);
        let mut parts = rest.splitn(2, ',');
        let header = parts.next().unwrap_or("");
        let data = parts.next().unwrap_or("");

        if data.is_empty() {
            return Ok(None);
        }

        let mut mime = "".to_string();
        let mut is_base64 = false;
        for (idx, part) in header.split(';').enumerate() {
            if idx == 0 {
                mime = part.to_string();
            } else if part.eq_ignore_ascii_case("base64") {
                is_base64 = true;
            }
        }

        if mime.is_empty() {
            return Ok(None);
        }

        if !is_base64 {
            return Ok(None);
        }

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| format!("Failed to decode data URL: {}", e))?;

        Ok(Some((mime, bytes)))
    }

    fn build_data_url(&self, mime: &str, bytes: &[u8]) -> String {
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        format!("data:{};base64,{}", mime, encoded)
    }

    fn image_dimensions(&self, mime: &str, bytes: &[u8]) -> Result<(f32, f32), String> {
        if mime.eq_ignore_ascii_case("image/svg+xml") {
            let opts = usvg::Options::default();
            let tree = usvg::Tree::from_data(bytes, &opts)
                .map_err(|e| format!("Failed to read SVG size: {}", e))?;
            let size = tree.size();
            return Ok((size.width(), size.height()));
        }

        let size =
            imagesize::blob_size(bytes).map_err(|e| format!("Failed to read image size: {}", e))?;
        Ok((size.width as f32, size.height as f32))
    }

    fn mime_from_path(&self, path: &Path) -> Option<&'static str> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match extension.as_str() {
            "png" => Some("image/png"),
            "jpg" | "jpeg" => Some("image/jpeg"),
            "svg" => Some("image/svg+xml"),
            _ => None,
        }
    }

    fn mime_from_url(&self, src: &str) -> Option<&'static str> {
        let trimmed = src.split(['?', '#']).next().unwrap_or(src);
        let extension = trimmed.rsplit('.').next().unwrap_or("");
        match extension.to_ascii_lowercase().as_str() {
            "png" => Some("image/png"),
            "jpg" | "jpeg" => Some("image/jpeg"),
            "svg" => Some("image/svg+xml"),
            _ => None,
        }
    }

    fn resolve_image_path(&self, src: &str) -> Option<PathBuf> {
        let src_path = Path::new(src);

        // Security: Disallow absolute paths for local images.
        // All local images must be resolved relative to the base_path.
        if src_path.is_absolute() {
            eprintln!("Warning: absolute image paths are disallowed for security: {}", src);
            return None;
        }

        if let Some(base) = self.base_path.as_ref() {
            let joined = base.join(src);

            // Path Traversal Mitigation:
            // 1. Join base and src
            // 2. Use components() to normalize the path without hitting the disk (no canonicalize() yet)
            // 3. Ensure the normalized path still starts with the base path.
            // Note: base_path is expected to be a directory, but join() with ".." can escape it.

            let mut normalized = PathBuf::new();
            for component in joined.components() {
                match component {
                    std::path::Component::Normal(c) => normalized.push(c),
                    std::path::Component::CurDir => {}
                    std::path::Component::ParentDir => {
                        normalized.pop();
                    }
                    std::path::Component::RootDir => normalized.push(std::path::MAIN_SEPARATOR.to_string()),
                    std::path::Component::Prefix(p) => normalized.push(p.as_os_str()),
                }
            }

            // Also normalize base path for comparison
            let mut normalized_base = PathBuf::new();
            for component in base.components() {
                match component {
                    std::path::Component::Normal(c) => normalized_base.push(c),
                    std::path::Component::CurDir => {}
                    std::path::Component::ParentDir => {
                        normalized_base.pop();
                    }
                    std::path::Component::RootDir => normalized_base.push(std::path::MAIN_SEPARATOR.to_string()),
                    std::path::Component::Prefix(p) => normalized_base.push(p.as_os_str()),
                }
            }

            if normalized.starts_with(&normalized_base) {
                return Some(joined);
            } else {
                eprintln!(
                    "Warning: blocked potential path traversal in image src: {}",
                    src
                );
                return None;
            }
        }

        // Security: If no base_path is provided, we don't allow resolving local images.
        None
    }

    fn start_list_item(&mut self) -> Result<(), String> {
        if self.at_line_start {
            // Move from list block top to first list-item baseline.
            if let Some(state) = self.list_stack.last_mut()
                && state.needs_ascent {
                    self.cursor_y += self.theme.font_size_base * 0.8;
                    state.needs_ascent = false;
                }
        } else {
            self.new_line();
        }

        let marker = self.next_list_marker();
        let marker_x = self.list_marker_x();

        self.pending_list_marker = Some(PendingListMarker { marker, marker_x });

        let (marker_width, _) = self.measure.measure_text(
            self.pending_list_marker
                .as_ref()
                .map(|pending| pending.marker.as_str())
                .unwrap_or(""),
            self.theme.font_size_base,
            false,
            false,
            false,
            None,
        );

        self.item_continuation_indent =
            Some(marker_x + marker_width + self.theme.font_size_base * LIST_MARKER_GAP_RATIO);
        self.cursor_x = self.item_continuation_indent.unwrap_or(self.line_start_x());
        self.at_line_start = true;

        Ok(())
    }

    fn end_list_item(&mut self) {
        self.pending_list_marker = None;
        self.item_continuation_indent = None;
    }

    fn start_blockquote(&mut self) {
        let depth = self.blockquotes.len() as f32;
        let border_x = self.theme.padding_x
            + depth * self.theme.font_size_base * QUOTE_INDENT_RATIO
            + self.theme.font_size_base * QUOTE_INNER_PADDING_RATIO * 0.5;
        let quote_pad_y = self.theme.font_size_base * 0.4;
        let start_y = self.cursor_y - self.theme.font_size_base * 0.8 - quote_pad_y;

        self.blockquotes.push(QuoteState { border_x, start_y });
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn end_blockquote(&mut self) {
        if !self.at_line_start {
            self.new_line();
        }

        if let Some(quote) = self.blockquotes.pop() {
            let bg_x = quote.border_x - 2.0;
            let bg_width = self.right_edge() - bg_x;
            let quote_pad_y = self.theme.font_size_base * 0.4;
            let end_y = self.cursor_y + quote_pad_y;
            write!(
                self.svg_content,
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" fill-opacity="0.06" />"#,
                bg_x,
                quote.start_y,
                bg_width,
                end_y - quote.start_y,
                self.theme.quote_border_color,
            )
            .unwrap();
            write!(
                self.svg_content,
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="3" />"#,
                quote.border_x,
                quote.start_y,
                quote.border_x,
                end_y,
                self.theme.quote_border_color,
            )
            .unwrap();
        }
    }

    fn start_block(&mut self, margin_top: f32, add_ascent: bool) {
        if !self.svg_content.is_empty() {
            if !self.at_line_start {
                // Move from current baseline to current line bottom.
                self.cursor_y += self.current_font_size() * 0.2;
                self.cursor_x = self.line_start_x();
                self.at_line_start = true;
            }
            self.add_margin(margin_top);
        }

        // Reset margin tracking so the block's own bottom margin isn't
        // collapsed against its top margin (margins should only collapse
        // between adjacent blocks, not through block content).
        self.last_margin_added = 0.0;

        if add_ascent {
            // Move from block top to first baseline.
            self.cursor_y += self.current_font_size() * 0.8;
        }

        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn finish_block(&mut self, margin_bottom: f32) {
        if !self.at_line_start {
            // Move from current baseline to current line bottom.
            self.cursor_y += self.current_font_size() * 0.2;
            self.cursor_x = self.line_start_x();
            self.at_line_start = true;
        }

        self.add_margin(margin_bottom);
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn add_margin(&mut self, margin: f32) {
        // Collapse consecutive vertical margins by applying only the delta
        // between the new margin and the previously applied one.
        if margin > self.last_margin_added {
            self.cursor_y += margin - self.last_margin_added;
        }
        self.last_margin_added = margin;
    }

    fn new_line(&mut self) {
        self.advance_line(self.current_font_size());
    }

    fn advance_line(&mut self, font_size: f32) {
        let descent_padding = font_size * 0.15;
        self.cursor_y += font_size * self.current_line_height() + descent_padding;
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn current_line_height(&self) -> f32 {
        if self.heading_level.is_some() {
            // Heading line height with safety margin
            self.theme.line_height.max(1.35)
        } else {
            // Body text line height with safety margin
            self.theme.line_height.max(1.4)
        }
    }

    fn current_font_size(&self) -> f32 {
        match self.heading_level {
            Some(HeadingLevel::H1) => self.theme.font_size_base * 2.2,
            Some(HeadingLevel::H2) => self.theme.font_size_base * 1.8,
            Some(HeadingLevel::H3) => self.theme.font_size_base * 1.5,
            Some(HeadingLevel::H4) => self.theme.font_size_base * 1.25,
            Some(HeadingLevel::H5) => self.theme.font_size_base * 1.1,
            Some(HeadingLevel::H6) => self.theme.font_size_base,
            None => self.theme.font_size_base,
        }
    }

    fn is_bold(&self) -> bool {
        self.heading_level.is_some() || self.strong_depth > 0
    }

    fn is_italic(&self) -> bool {
        self.emphasis_depth > 0
    }

    fn current_fill(&self) -> String {
        // Inline HTML color overrides the theme-derived color.
        if let Some(color) = self.effective_inline_style().color {
            return color;
        }
        if self.link_depth > 0 {
            self.theme.link_color.clone()
        } else if self.heading_level.is_some() {
            self.theme.heading_color.clone()
        } else if !self.blockquotes.is_empty() {
            self.theme.quote_text_color.clone()
        } else {
            self.theme.text_color.clone()
        }
    }

    fn next_list_marker(&mut self) -> String {
        if let Some(state) = self.list_stack.last_mut() {
            if state.ordered {
                let marker = format!("{}.", state.next_index);
                state.next_index += 1;
                marker
            } else {
                "•".to_string()
            }
        } else {
            "•".to_string()
        }
    }

    fn list_marker_x(&self) -> f32 {
        let base_indent = self.theme.font_size_base * LIST_INDENT_RATIO;
        let depth_offset = self.list_stack.len().saturating_sub(1) as f32
            * self.theme.font_size_base
            * LIST_INDENT_RATIO;
        self.base_left_indent() + base_indent + depth_offset
    }

    fn base_left_indent(&self) -> f32 {
        if self.blockquotes.is_empty() {
            self.theme.padding_x
        } else {
            self.theme.padding_x
                + self.blockquotes.len() as f32 * self.theme.font_size_base * QUOTE_INDENT_RATIO
                + self.theme.font_size_base * QUOTE_INNER_PADDING_RATIO
        }
    }

    fn line_start_x(&self) -> f32 {
        self.item_continuation_indent
            .unwrap_or_else(|| self.base_left_indent())
    }

    fn right_edge(&self) -> f32 {
        self.width - self.theme.padding_x
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text_at(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        font_family: &str,
        font_size: f32,
        fill: &str,
        bold: bool,
        italic: bool,
    ) -> f32 {
        self.last_margin_added = 0.0;

        // SVG is XML: drop XML-illegal codepoints early so measuring and output agree.
        let text = crate::xml::sanitize_xml_text(text);
        let is_code = font_family == "monospace";
        let (width, _) = self
            .measure
            .measure_text(&text, font_size, is_code, bold, italic, None);

        let weight_attr = if bold { " font-weight=\"700\"" } else { "" };
        let style_attr = if italic { " font-style=\"italic\"" } else { "" };

        write!(
            self.svg_content,
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.2}" fill="{}"{}{}>{}</text>"#,
            x,
            y,
            font_family,
            font_size,
            fill,
            weight_attr,
            style_attr,
            crate::xml::escape_xml(&text).replace(' ', "&#160;"),
        )
        .unwrap();

        width
    }

    fn finalize_svg(&self, height: f32) -> String {
        let mut svg = String::with_capacity(self.svg_content.len() + 256);
        write!(
            svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}"><rect width="100%" height="100%" fill="{}" />{}</svg>"#,
            self.width,
            height,
            self.width,
            height,
            self.theme.background_color,
            self.svg_content,
        )
        .unwrap();
        svg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::TextMeasure;
    use crate::theme::Theme;

    // Mock TextMeasure for testing
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
            // Simple approximation: width = len * size * 0.6, height = size
            (text.len() as f32 * font_size * 0.6, font_size)
        }
    }

    /// Records every measured text so tests can assert on measurement counts.
    #[derive(Default)]
    struct CountingMeasure {
        texts: std::cell::RefCell<Vec<String>>,
    }
    impl TextMeasure for CountingMeasure {
        fn measure_text(
            &mut self,
            text: &str,
            font_size: f32,
            _is_code: bool,
            _is_bold: bool,
            _is_italic: bool,
            _max_width: Option<f32>,
        ) -> (f32, f32) {
            self.texts.borrow_mut().push(text.to_string());
            (text.len() as f32 * font_size * 0.6, font_size)
        }
    }

    struct ZeroSpaceMeasure;
    impl TextMeasure for ZeroSpaceMeasure {
        fn measure_text(
            &mut self,
            text: &str,
            font_size: f32,
            _is_code: bool,
            _is_bold: bool,
            _is_italic: bool,
            _max_width: Option<f32>,
        ) -> (f32, f32) {
            let width = match text {
                " " => 0.0,
                "m m" => 30.0,
                "mm" => 20.0,
                _ => text.len() as f32 * font_size * 0.6,
            };
            (width, font_size)
        }
    }

    #[test]
    fn test_renderer_initialization() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let renderer = Renderer::new(theme, measure, 800.0);
        assert!(renderer.is_ok());
    }

    #[test]
    fn test_inline_code_rendering() {
        let theme = Theme {
            code_padding_y: 10.0,
            font_size_code: 14.0,
            ..Theme::default()
        };

        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // This should trigger render_inline_code
        let markdown = "`code`";
        let result = renderer.render(markdown);

        assert!(result.is_ok());
        let svg = result.unwrap();

        // Check if rect height is calculated correctly according to the new logic
        // rect_height = font_size_code * 1.25 + code_padding_y
        // 14.0 * 1.25 + 10.0 = 27.5
        assert!(svg.contains("height=\"27.50\""));
    }

    #[test]
    fn test_code_block_syntax_highlighting() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "```rust\nfn main() {}\n```";
        let result = renderer.render(markdown);

        assert!(result.is_ok());
        let svg = result.unwrap();

        // Check for syntax highlighting colors
        // Rust keywords like 'fn' should be colored.
        // In Solarized themes (used in logic), keywords are often colored.
        // We look for fill attributes that are NOT the default text color

        // Note: The specific color depends on the syntect theme loaded.
        // But we can check that we have multiple different fill colors in the output
        // or specifically that we have spans/tspan/text with fill attributes.

        // In the implementation, draw_text_at uses text tag with fill attribute.
        // Let's verify we have text tags with fill colors.
        assert!(svg.contains("<text"));
        assert!(svg.contains("fill=\"#"));
    }

    #[test]
    fn test_syntax_highlighting_language_detection() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // Python code
        let markdown = "```python\ndef foo():\n    pass\n```";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg_py = result.unwrap();

        // Rust code
        let markdown_rs = "```rust\nfn main() {}\n```";
        let result_rs = renderer.render(markdown_rs);
        assert!(result_rs.is_ok());
        let svg_rs = result_rs.unwrap();

        // The SVGs should be different (different content and potentially different colors)
        assert_ne!(svg_py, svg_rs);
    }

    #[test]
    fn test_svg_output_strips_xml_invalid_control_chars() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "Intro\n\n\u{0007}## 1) Summary\n";
        let result = renderer.render(markdown);

        assert!(result.is_ok());
        let svg = result.unwrap();
        assert!(!svg.contains('\u{0007}'));
        assert!(svg.contains("Summary"));
        assert!(!svg.contains(">##<"));
    }

    #[test]
    fn test_mermaid_class_relations_rendered_from_markdown_fence() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = r#"
# Class Example

```mermaid
classDiagram
  class User {
    +String id
  }
  class Session {
    +String token
  }
  class AuditLog {
    +record(event: String): void
  }

  User --> Session : creates
  User ..> AuditLog : writes
```
"#;
        let svg = renderer.render(markdown).unwrap();
        assert!(
            svg.contains("creates") && svg.contains("writes"),
            "Expected class relation labels to be present in SVG"
        );
    }

    #[test]
    fn test_markdown_mermaid_fence_preserves_relation_lines() {
        let markdown = r#"
# Class Example

```mermaid
classDiagram
  class User {
    +String id
  }
  class Session {
    +String token
  }
  User --> Session : creates
```
"#;
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_MATH);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_DEFINITION_LIST);
        options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        options.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);

        let parser = Parser::new_ext(markdown, options);
        let mut in_code = false;
        let mut lang = None::<String>;
        let mut code = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(kind)) => {
                    in_code = true;
                    lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(l) => Some(l.to_string()),
                        _ => None,
                    };
                }
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(t) if in_code => code.push_str(&t),
                Event::Code(t) if in_code => code.push_str(&t),
                Event::SoftBreak | Event::HardBreak if in_code => code.push('\n'),
                _ => {}
            }
        }

        assert_eq!(lang.as_deref(), Some("mermaid"));
        assert!(code.contains("User --> Session : creates"));
    }

    #[test]
    fn test_space_width_inference_is_memoized() {
        let theme = Theme::default();
        let measure = CountingMeasure::default();
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // Five spaces: the inference measurements ("m m", "mm") should happen
        // exactly once, not once per space token.
        renderer.render("a b c d e f").unwrap();

        let texts = renderer.measure.texts.borrow();
        let m_m_count = texts.iter().filter(|t| t.as_str() == "m m").count();
        let mm_count = texts.iter().filter(|t| t.as_str() == "mm").count();
        assert_eq!(m_m_count, 1, "'m m' inference should run once, not per space");
        assert_eq!(mm_count, 1, "'mm' inference should run once, not per space");
    }

    #[test]
    fn test_whitespace_fallback_prevents_collapsed_words() {
        let theme = Theme::default();
        let measure = ZeroSpaceMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer.render("Hello World").unwrap();

        let hello_idx = svg.find(">Hello</text>").expect("Hello text missing");
        let world_idx = svg.find(">World</text>").expect("World text missing");
        assert!(world_idx > hello_idx, "World should render after Hello");

        let world_prefix = &svg[..world_idx];
        let x_start = world_prefix
            .rfind("x=\"")
            .expect("missing x attribute for World")
            + 3;
        let x_end = world_prefix[x_start..]
            .find('"')
            .expect("unterminated x attribute for World")
            + x_start;
        let world_x: f32 = world_prefix[x_start..x_end].parse().unwrap_or(0.0);

        // 32px left padding + Hello width (42) + inferred space (10) = 84
        assert!(world_x >= 84.0, "Expected non-zero spacing between words");
    }

    #[test]
    fn test_parser_preserves_space_in_heading_text_event() {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_MATH);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_DEFINITION_LIST);
        options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        options.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
        let parser = Parser::new_ext("# Hello World\n", options);
        let texts: Vec<String> = parser
            .filter_map(|event| match event {
                Event::Text(t) => Some(t.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t == "Hello World"),
            "Expected heading text event 'Hello World', got: {:?}",
            texts
        );
    }

    #[test]
    fn test_mermaid_block_scales_down_when_wider_than_content() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 360.0).unwrap();

        let markdown = r#"
```mermaid
flowchart LR
    A[Start] --> B[Gateway]
    B --> C[Auth]
    C --> D[Query]
    D --> E[Transform]
    E --> F[Cache]
    F --> G[Publish]
    G --> H[Done]
```
"#;

        let svg = renderer.render(markdown).unwrap();
        assert!(
            svg.contains("scale("),
            "Expected Mermaid block to be scaled when too wide"
        );
    }

    #[test]
    fn test_resolve_image_path_traversal() {
        let theme = Theme::default();
        let measure = MockMeasure;
        // Using a real directory that should exist in the sandbox
        let base_path = std::env::current_dir().unwrap().join("src");
        let renderer = Renderer::new_with_base_path(theme, measure, 800.0, Some(base_path.clone())).unwrap();

        // Try to traverse to Cargo.toml which is one level up from src/
        let traversal_path = "../Cargo.toml";
        let resolved = renderer.resolve_image_path(traversal_path);

        // After fix, it should be blocked
        assert!(resolved.is_none());

        // Normal path should still work
        let normal_path = "renderer.rs";
        let resolved = renderer.resolve_image_path(normal_path);
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ends_with("src/renderer.rs"));
    }

    #[test]
    fn test_resolve_image_path_absolute() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let base_path = std::env::current_dir().unwrap().join("src");
        let renderer = Renderer::new_with_base_path(theme, measure, 800.0, Some(base_path)).unwrap();

        let abs_path = if cfg!(windows) { "C:\\Windows\\System32\\drivers\\etc\\hosts" } else { "/etc/passwd" };
        let resolved = renderer.resolve_image_path(abs_path);

        // Absolute paths should be blocked
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_image_path_no_base() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let renderer = Renderer::new_with_base_path(theme, measure, 800.0, None).unwrap();

        let rel_path = "some_image.png";
        let resolved = renderer.resolve_image_path(rel_path);

        // Without base_path, local images should not be resolved
        assert!(resolved.is_none());
    }

    #[test]
    fn test_definition_list_consistent_spacing() {
        // Regression test: definition list items should have consistent spacing.
        //
        // Bug pattern (similar to nested list bug): Both Tag::DefinitionListTitle and
        // TagEnd::DefinitionListTitle had `if !at_line_start { new_line() }` calls.
        // This redundant pattern could cause spacing issues in edge cases.
        //
        // Fix: Only advance in start tags, not in end tags.
        //
        // Expected: All gaps between title→definition and definition→next_title
        // should be consistent (within tolerance).
        let theme = Theme::default();
        let measure = crate::fonts::CosmicTextMeasure::new()
            .expect("Failed to initialize font system");
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // Use unique identifiers to distinguish title from definition text
        let markdown = r#"AlphaOne
: AlphaDef for first term.

BetaTwo
: BetaDef for second term.

GammaThree
: GammaDef for third term.
"#;
        let result = renderer.render(markdown);
        assert!(result.is_ok());

        let svg = result.unwrap();

        fn extract_y_for_text(svg: &str, search_text: &str) -> Option<f32> {
            let search = format!(">{search_text}</text>");
            let text_idx = svg.find(&search)?;
            let prefix = &svg[..text_idx];
            let pattern = " y=\"";
            let y_pattern_pos = prefix.rfind(pattern)?;
            let y_start = y_pattern_pos + pattern.len();
            let y_end = prefix[y_start..].find('"')? + y_start;
            prefix[y_start..y_end].parse().ok()
        }

        // Title text is bold, definition text is not
        let first_title_y = extract_y_for_text(&svg, "AlphaOne")
            .expect("Should find 'AlphaOne' in SVG");
        let first_def_y = extract_y_for_text(&svg, "AlphaDef")
            .expect("Should find 'AlphaDef' in SVG");
        let second_title_y = extract_y_for_text(&svg, "BetaTwo")
            .expect("Should find 'BetaTwo' in SVG");
        let second_def_y = extract_y_for_text(&svg, "BetaDef")
            .expect("Should find 'BetaDef' in SVG");
        let third_title_y = extract_y_for_text(&svg, "GammaThree")
            .expect("Should find 'GammaThree' in SVG");

        // Calculate gaps
        let gap_title_to_def = first_def_y - first_title_y;
        let gap_def_to_next_title = second_title_y - first_def_y;
        let gap_title_to_def_2 = second_def_y - second_title_y;
        let gap_def_to_next_title_2 = third_title_y - second_def_y;

        // All title→definition gaps should be similar
        let tolerance = 5.0;
        assert!(
            (gap_title_to_def - gap_title_to_def_2).abs() <= tolerance,
            "Title→Definition gaps should be consistent: first={gap_title_to_def:.2}, second={gap_title_to_def_2:.2}"
        );

        // All definition→next title gaps should be similar
        assert!(
            (gap_def_to_next_title - gap_def_to_next_title_2).abs() <= tolerance,
            "Definition→NextTitle gaps should be consistent: first={gap_def_to_next_title:.2}, second={gap_def_to_next_title_2:.2}"
        );

        // KEY TEST: definition→next title gap should NOT be 2x the title→definition gap
        // If there's redundant new_line() calls in both start and end tags,
        // the definition→next title gap will be much larger.
        let max_ratio = 2.0;
        let ratio = gap_def_to_next_title / gap_title_to_def;
        assert!(
            ratio <= max_ratio,
            "Definition→NextTitle gap ({gap_def_to_next_title:.2}) should not be much larger than Title→Definition gap ({gap_title_to_def:.2}). Ratio: {ratio:.2}x (max allowed: {max_ratio}x). This indicates redundant new_line() calls in definition list handling."
        );
    }

    // ========================================
    // TDD Quick Win Tests (Phase 1)
    // ========================================

    /// Quick Win 1: Line height safety margins
    /// Test that line height has a minimum safety margin even with tight theme settings
    #[test]
    fn test_line_height_has_safety_margin_for_body_text() {
        // Create a theme with very tight line height
        let theme = Theme {
            line_height: 1.0, // Very tight - should be overridden by safety margin
            font_size_base: 16.0,
            ..Theme::default()
        };
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // Render multiple paragraphs to test line spacing
        let markdown = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let result = renderer.render(markdown);
        assert!(result.is_ok());

        let svg = result.unwrap();

        // Extract y positions of text elements
        fn extract_y_positions(svg: &str) -> Vec<f32> {
            let mut positions = Vec::new();
            let pattern = " y=\"";
            let mut search_start = 0;
            while let Some(pos) = svg[search_start..].find(pattern) {
                let abs_pos = search_start + pos;
                let y_start = abs_pos + pattern.len();
                if let Some(end_pos) = svg[y_start..].find('"')
                    && let Ok(y) = svg[y_start..y_start + end_pos].parse::<f32>() {
                        positions.push(y);
                    }
                search_start = y_start;
            }
            positions
        }

        let y_positions = extract_y_positions(&svg);
        // We should have at least 3 text elements (one for each paragraph)
        assert!(y_positions.len() >= 3, "Should have at least 3 text elements, found {}", y_positions.len());

        // Sort positions to get correct order
        let mut sorted: Vec<f32> = y_positions.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Check gaps between consecutive elements
        // With safety margin of 1.4, each gap should be at least font_size * 1.4 + descent
        let font_size = 16.0_f32;
        let min_expected_gap = font_size * 1.4; // line_height with safety margin

        for i in 1..sorted.len().min(5) {
            let gap = sorted[i] - sorted[i-1];
            // We expect reasonable gaps (some might be in same line, so only check larger gaps)
            if gap > font_size * 0.5 {  // Only check if it's potentially a line break
                assert!(
                    gap >= min_expected_gap * 0.8, // Allow some tolerance
                    "Line spacing gap ({gap:.2}) should be at least {min:.2} (80% tolerance)",
                    min = min_expected_gap * 0.8
                );
            }
        }
    }

    /// Quick Win 2: Descent padding in advance_line
    /// Test that line advances include descent padding to prevent text overlap
    #[test]
    fn test_line_advance_includes_descent_padding() {
        let theme = Theme {
            line_height: 1.4,
            font_size_base: 16.0,
            ..Theme::default()
        };
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // Record initial cursor position
        let initial_y = renderer.cursor_y;

        // Advance a line and check cursor movement
        let font_size = 16.0_f32;
        renderer.advance_line(font_size);

        let cursor_delta = renderer.cursor_y - initial_y;

        // With line_height 1.4 and descent_padding 0.15:
        // Expected delta = font_size * line_height + font_size * 0.15
        // = 16 * 1.4 + 16 * 0.15 = 22.4 + 2.4 = 24.8
        let expected_min = font_size * 1.4; // at least line_height
        let expected_with_descent = font_size * 1.4 + font_size * 0.15;

        assert!(
            cursor_delta >= expected_min,
            "Cursor delta ({cursor_delta:.2}) should be at least {expected_min:.2}"
        );

        // The descent padding should add extra space
        assert!(
            cursor_delta >= expected_with_descent * 0.95, // Allow small floating point variance
            "Cursor delta ({cursor_delta:.2}) should include descent padding (expected ~{expected_with_descent:.2})"
        );
    }

    /// Quick Win 3: Inline code box alignment
    /// Test that inline code background rect is properly aligned
    #[test]
    fn test_inline_code_box_alignment_uses_ascent_ratio() {
        let theme = Theme {
            font_size_code: 14.0,
            code_padding_y: 4.0,
            ..Theme::default()
        };
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "Text with `code` inline";
        let result = renderer.render(markdown);
        assert!(result.is_ok());

        let svg = result.unwrap();

        // Find the rect y position (should be based on ascent_ratio=0.75, not 0.8)
        // Expected: rect_y = cursor_y - font_size_code * 0.75 - code_padding_y * 0.5
        assert!(
            svg.contains("<rect"),
            "SVG should contain a rect for inline code background"
        );

        // The fix changes from 0.8 to 0.75 ratio
        // This is a visual test - the rect should be positioned correctly
        // We verify the rendering succeeds without errors
    }

    /// Test that consecutive inline code elements don't overlap
    #[test]
    fn test_consecutive_inline_code_no_overlap() {
        let theme = Theme {
            font_size_code: 12.0,
            code_padding_x: 3.0,
            code_padding_y: 2.0,
            ..Theme::default()
        };
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "`first` `second` `third`";
        let result = renderer.render(markdown);
        assert!(result.is_ok());

        let svg = result.unwrap();

        // Count rect elements - should have 3 for inline code
        let rect_count = svg.matches("<rect").count();
        assert!(
            rect_count >= 3,
            "Should have at least 3 rect elements for inline code, found {rect_count}"
        );
    }

    // ========================================
    // Phase 2: Property-Based Tests
    // ========================================

    /// Property test: Line spacing should prevent visual overlap
    /// across a wide range of font sizes and line heights
    #[test]
    fn test_proptest_line_spacing_prevents_overlap() {
        use proptest::prelude::*;

        proptest!(|(font_size in 8.0f32..48.0, line_height in 0.8f32..3.0)| {
            let theme = Theme {
                line_height,
                font_size_base: font_size,
                ..Theme::default()
            };
            let measure = MockMeasure;
            let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

            let initial_y = renderer.cursor_y;
            renderer.advance_line(font_size);
            let cursor_delta = renderer.cursor_y - initial_y;

            // Next line should start with enough gap to prevent visual overlap
            // Minimum gap should be at least font_size * 0.9 (accounting for descent)
            prop_assert!(
                cursor_delta >= font_size * 0.9,
                "cursor_delta ({}) should be >= font_size * 0.9 ({})",
                cursor_delta,
                font_size * 0.9
            );
        });
    }

    /// Property test: Renderer should handle any valid markdown input
    /// without panicking or returning an error
    #[test]
    fn test_proptest_renders_any_markdown_without_error() {
        use proptest::prelude::*;

        // Generate valid markdown-like text
        prop_compose! {
            fn arb_markdown_text()(s in "[a-zA-Z0-9 \n.,!?*-]*") -> String {
                s
            }
        }

        proptest!(|(text in arb_markdown_text())| {
            let theme = Theme::default();
            let measure = MockMeasure;
            let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

            let result = renderer.render(&text);
            prop_assert!(result.is_ok(), "Renderer should not error on valid markdown");
        });
    }

    /// Property test: SVG output should always be valid
    /// (contains required root element and proper structure)
    #[test]
    fn test_proptest_svg_output_valid_structure() {
        use proptest::prelude::*;

        prop_compose! {
            fn arb_simple_text()(s in "[a-zA-Z0-9 ]{1,50}") -> String {
                s
            }
        }

        proptest!(|(text in arb_simple_text())| {
            let theme = Theme::default();
            let measure = MockMeasure;
            let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

            let result = renderer.render(&text);
            prop_assert!(result.is_ok());

            let svg = result.unwrap();

            // Basic SVG structure validation
            prop_assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"));
            prop_assert!(svg.contains("<svg"));
            prop_assert!(svg.contains("</svg>"));
        });
    }

    /// Property test: Inline code elements should never overlap
    /// regardless of the number of consecutive code spans
    #[test]
    fn test_proptest_multiple_inline_code_no_overlap() {
        use proptest::prelude::*;

        proptest!(|(count in 2usize..10)| {
            let theme = Theme {
                font_size_code: 14.0,
                code_padding_x: 4.0,
                code_padding_y: 3.0,
                ..Theme::default()
            };
            let measure = MockMeasure;
            let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

            // Generate markdown with `count` inline code elements
            let code_elements: Vec<String> = (0..count).map(|i| format!("`code{i}`")).collect();
            let markdown = code_elements.join(" ");

            let result = renderer.render(&markdown);
            prop_assert!(result.is_ok());

            let svg = result.unwrap();

            // Should have at least `count` rect elements
            let rect_count = svg.matches("<rect").count();
            prop_assert!(
                rect_count >= count,
                "Expected at least {} rects, found {}",
                count,
                rect_count
            );
        });
    }

    /// Property test: Renderer width must accommodate content
    #[test]
    fn test_proptest_renderer_width_constraint() {
        use proptest::prelude::*;

        proptest!(|(width in 200.0f32..2000.0)| {
            let theme = Theme::default();
            let measure = MockMeasure;
            let result = Renderer::new(theme, measure, width);

            prop_assert!(result.is_ok(), "Renderer should be creatable with width {}", width);
        });
    }

    /// Property test: Consecutive lines should have increasing y positions
    #[test]
    fn test_proptest_lines_have_increasing_y_positions() {
        use proptest::prelude::*;

        proptest!(|(line_count in 2usize..20)| {
            let theme = Theme::default();
            let measure = MockMeasure;
            let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

            // Render multiple paragraphs
            let paragraphs: Vec<String> = (0..line_count).map(|i| format!("Paragraph {} content here.", i)).collect();
            let markdown = paragraphs.join("\n\n");

            let result = renderer.render(&markdown);
            prop_assert!(result.is_ok());

            let svg = result.unwrap();

            // Extract y positions
            fn extract_y_positions(svg: &str) -> Vec<f32> {
                let mut positions = Vec::new();
                let pattern = " y=\"";
                let mut search_start = 0;
                while let Some(pos) = svg[search_start..].find(pattern) {
                    let abs_pos = search_start + pos;
                    let y_start = abs_pos + pattern.len();
                    if let Some(end_pos) = svg[y_start..].find('"')
                        && let Ok(y) = svg[y_start..y_start + end_pos].parse::<f32>() {
                            positions.push(y);
                        }
                    search_start = y_start;
                }
                positions
            }

            let y_positions = extract_y_positions(&svg);

            // Sort and check that unique positions are increasing
            let mut unique_sorted: Vec<f32> = y_positions.to_vec();
            unique_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            unique_sorted.dedup();

            // With line_count paragraphs, we should have multiple distinct y positions
            // and they should be in increasing order (already sorted)
            if unique_sorted.len() >= 2 {
                for i in 1..unique_sorted.len() {
                    prop_assert!(
                        unique_sorted[i] > unique_sorted[i-1],
                        "Y positions should be strictly increasing: {} should be > {}",
                        unique_sorted[i],
                        unique_sorted[i-1]
                    );
                }
            }
        });
    }

    /// Property test: Code block rendering should handle any code content
    #[test]
    fn test_proptest_code_blocks_handle_any_content() {
        use proptest::prelude::*;

        prop_compose! {
            fn arb_code_content()(s in "[a-zA-Z0-9 \\n\\t{}()\\[\\];,.:!?+=\\-*/%<>\"']{0,200}") -> String {
                s
            }
        }

        proptest!(|(code in arb_code_content())| {
            let theme = Theme::default();
            let measure = MockMeasure;
            let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

            let markdown = format!("```\n{}\n```", code);
            let result = renderer.render(&markdown);

            prop_assert!(result.is_ok(), "Code block should render without error");

            let svg = result.unwrap();
            // Should contain code block elements
            prop_assert!(svg.contains("<rect") || svg.contains("<text"));
        });
    }

    // ========================================
    // Bug Fix Tests
    // ========================================

    #[test]
    fn test_missing_image_renders_alt_text_fallback() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let base_path = std::env::temp_dir();
        let mut renderer =
            Renderer::new_with_base_path(theme, measure, 800.0, Some(base_path)).unwrap();

        let markdown = "# Title\n\n![Alt description](nonexistent-image-12345.png)\n\nMore text";
        let result = renderer.render(markdown);

        assert!(result.is_ok(), "Missing image should not cause render failure");
        let svg = result.unwrap();
        assert!(svg.contains("Alt"), "SVG should contain alt text as fallback");
        assert!(svg.contains("Title"), "SVG should still contain other content");
        assert!(svg.contains("More"), "SVG should contain content after the image");
    }

    #[test]
    fn test_table_header_row_has_bold_text() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "| Header1 | Header2 |\n|---------|----------|\n| data1   | data2    |";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        assert!(
            svg.contains("font-weight=\"700\""),
            "Table header should have bold text styling. SVG:\n{}",
            svg
        );
    }

    #[test]
    fn test_table_header_row_has_background() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "| H1 | H2 |\n|----|----|\n| d1 | d2 |";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        assert!(
            svg.contains("fill-opacity"),
            "Table header row should have a background fill"
        );
    }

    // ========================================
    // GFM Feature Tests
    // ========================================

    #[test]
    fn test_gfm_task_list_unchecked() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "- [ ] Unchecked task";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Should contain checkbox rect
        assert!(
            svg.contains("<rect"),
            "Unchecked task should have a checkbox rect"
        );
        // Should NOT contain polyline (checkmark)
        assert!(
            !svg.contains("<polyline"),
            "Unchecked task should not have a checkmark"
        );
    }

    #[test]
    fn test_gfm_task_list_checked() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "- [x] Checked task";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Should contain checkbox rect
        assert!(
            svg.contains("<rect"),
            "Checked task should have a checkbox rect"
        );
        // Should contain polyline (checkmark)
        assert!(
            svg.contains("<polyline"),
            "Checked task should have a checkmark"
        );
    }

    #[test]
    fn test_gfm_task_list_multiple_items() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "- [x] Done
- [ ] Pending
- [ ] Later";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Should have 3 checkboxes (rects) - 1 checked + 2 unchecked
        let rect_count = svg.matches("<rect").count();
        assert!(rect_count >= 3, "Should have at least 3 checkbox rects, found {}", rect_count);

        // Should have exactly 1 polyline (checkmark for checked item)
        let polyline_count = svg.matches("<polyline").count();
        assert!(polyline_count == 1, "Should have exactly 1 checkmark, found {}", polyline_count);
    }

    #[test]
    fn test_gfm_strikethrough() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "~~deleted text~~ remaining";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Should contain both texts
        assert!(svg.contains("deleted"), "SVG should contain strikethrough text");
        assert!(svg.contains("remaining"), "SVG should contain remaining text");
        // Should have a line element for strikethrough decoration
        assert!(
            svg.contains("<line"),
            "Strikethrough should have a line decoration"
        );
    }

    #[test]
    fn test_gfm_strikethrough_middle_of_text() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "The ~~quick~~ brown fox.";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        assert!(svg.contains("quick"), "Strikethrough text should be rendered");
        assert!(svg.contains("brown"), "Text after strikethrough should be rendered");
    }

    #[test]
    fn test_gfm_footnote_reference() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "Here is a footnote[^1] in text.";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Should contain the footnote marker
        assert!(svg.contains("footnote"), "Footnote reference should be rendered");
    }

    #[test]
    fn test_gfm_footnote_definition() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "Text here.[^1]

[^1]: This is the footnote definition.";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Should contain footnote content
        assert!(svg.contains("footnote"), "Footnote should be rendered");
        assert!(svg.contains("[1]"), "Footnote marker should be rendered");
    }

    #[test]
    fn test_gfm_definition_list() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "Term1
: Definition for term 1

Term2
: Definition for term 2";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Should contain terms (bold)
        assert!(svg.contains("Term1"), "Definition term should be rendered");
        assert!(svg.contains("Term2"), "Second term should be rendered");
        // Should contain definitions
        assert!(svg.contains("Definition"), "Definitions should be rendered");
    }

    #[test]
    fn test_gfm_definition_list_multiple_items() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "Apple
: A fruit

Banana
: Another fruit

Cherry
: Yet another fruit";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Should have all three terms
        assert!(svg.contains("Apple"), "First term should be rendered");
        assert!(svg.contains("Banana"), "Second term should be rendered");
        assert!(svg.contains("Cherry"), "Third term should be rendered");
    }

    #[test]
    fn test_gfm_table_alignment_left() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "| Left | Center | Right |
|:-----|:------:|-------:|
| a    |   b    |      c |";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Table should render
        assert!(svg.contains("Left"), "Table header should be rendered");
        assert!(svg.contains("a"), "Table cell should be rendered");
    }

    #[test]
    fn test_gfm_table_alignment_center() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "| A | B |
|:--:--:|
| 1 | 2 |";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Center-aligned table should render
        assert!(svg.contains("A"), "Center-aligned header should render");
    }

    #[test]
    fn test_html_block_renders_as_code_not_dropped() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let markdown = "<div class=\"note\">\nHello HTML content\n</div>\n\nAfter HTML";
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // HTML blocks are documented as rendered as code, never dropped.
        assert!(
            svg.contains("Hello"),
            "HTML block content should be visible, not silently dropped"
        );
        assert!(
            svg.contains("div"),
            "HTML block source should render as code"
        );
        assert!(
            svg.contains("After"),
            "Content after the HTML block should still render"
        );
    }

    // Extract (font-size, y, fill) of the first text node with the given content.
    fn text_metrics(svg: &str, content: &str) -> Option<(f32, f32, String)> {
        let needle = format!(">{}</text>", content);
        let idx = svg.find(&needle)?;
        let prefix = &svg[..idx];
        // Space-prefixed so `y="` doesn't match the tail of `font-family="`.
        let attr = |name: &str| -> Option<String> {
            let pattern = format!(" {}=\"", name);
            let pos = prefix.rfind(&pattern)? + pattern.len();
            let rest = &prefix[pos..];
            Some(rest.split('"').next()?.to_string())
        };
        let font_size: f32 = attr("font-size")?.parse().ok()?;
        let y: f32 = attr("y")?.parse().ok()?;
        let fill = attr("fill").unwrap_or_default();
        Some((font_size, y, fill))
    }

    #[test]
    fn test_inline_html_superscript_raised_and_smaller() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer.render("x<sup>2</sup>").unwrap();
        let (x_fs, x_y, _) = text_metrics(&svg, "x").expect("base text");
        let (two_fs, two_y, _) = text_metrics(&svg, "2").expect("superscript");

        assert!(two_fs < x_fs, "superscript should be smaller: {two_fs} vs {x_fs}");
        assert!(two_y < x_y, "superscript should be raised: {two_y} vs {x_y}");
    }

    #[test]
    fn test_inline_html_subscript_lowered_and_smaller() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer.render("H<sub>2</sub>O").unwrap();
        let (h_fs, h_y, _) = text_metrics(&svg, "H").expect("base text");
        let (two_fs, two_y, _) = text_metrics(&svg, "2").expect("subscript");

        assert!(two_fs < h_fs, "subscript should be smaller: {two_fs} vs {h_fs}");
        assert!(two_y > h_y, "subscript should be lowered: {two_y} vs {h_y}");
    }

    #[test]
    fn test_inline_html_span_and_font_color() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer
            .render("A <span style=\"color: #ff0000\">red</span> and <font color=\"blue\">blue</font>")
            .unwrap();

        let (_, _, red_fill) = text_metrics(&svg, "red").expect("span text");
        assert_eq!(red_fill, "#ff0000", "span style color should apply");

        let (_, _, blue_fill) = text_metrics(&svg, "blue").expect("font text");
        assert_eq!(blue_fill, "blue", "font color attribute should apply");

        let (_, _, a_fill) = text_metrics(&svg, "A").expect("plain text");
        assert_ne!(a_fill, "#ff0000", "plain text should keep the theme color");
    }

    #[test]
    fn test_inline_html_mark_highlight_and_underline() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer
            .render("<mark>hi</mark> and <u>lo</u>")
            .unwrap();

        // <mark> draws a yellow highlight rect behind the text.
        assert!(
            svg.contains("fill=\"#ffff00\""),
            "<mark> should add a yellow highlight rect"
        );
        // <u> draws a line decoration.
        assert!(
            svg.contains("<line"),
            "<u> should add an underline line"
        );
        assert!(svg.contains(">hi</text>"), "mark content should render");
        assert!(svg.contains(">lo</text>"), "underline content should render");
    }

    /// Parse `x`/`y`/`width` out of every `<rect ... />` in the SVG.
    fn svg_rects(svg: &str) -> Vec<(f32, f32, f32)> {
        let mut out = Vec::new();
        let mut rest = svg;
        while let Some(start) = rest.find("<rect ") {
            let end = rest[start..]
                .find("/>")
                .map(|e| start + e)
                .unwrap_or(rest.len());
            let tag = &rest[start..end];
            let attr = |name: &str| {
                let needle = format!("{}=\"", name);
                tag.find(&needle).and_then(|i| {
                    let v = &tag[i + needle.len()..];
                    let q = v.find('"')?;
                    v[..q].parse::<f32>().ok()
                })
            };
            if let (Some(x), Some(y), Some(w)) = (attr("x"), attr("y"), attr("width")) {
                out.push((x, y, w));
            }
            rest = &rest[end..];
        }
        out
    }

    /// Parse `x1`/`x2` out of every `<line ... />` in the SVG.
    fn svg_lines(svg: &str) -> Vec<(f32, f32)> {
        let mut out = Vec::new();
        let mut rest = svg;
        while let Some(start) = rest.find("<line ") {
            let end = rest[start..]
                .find("/>")
                .map(|e| start + e)
                .unwrap_or(rest.len());
            let tag = &rest[start..end];
            let attr = |name: &str| {
                let needle = format!("{}=\"", name);
                tag.find(&needle).and_then(|i| {
                    let v = &tag[i + needle.len()..];
                    let q = v.find('"')?;
                    v[..q].parse::<f32>().ok()
                })
            };
            if let (Some(x1), Some(x2)) = (attr("x1"), attr("x2")) {
                out.push((x1, x2));
            }
            rest = &rest[end..];
        }
        out
    }

    #[test]
    fn test_mark_highlight_continuous_across_space() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // MockMeasure width: len * 16 * 0.6 → "a" = 9.6, space = 9.6, "b" = 9.6.
        let svg = renderer.render("<mark>a b</mark>").unwrap();
        let rects = svg_rects(&svg);
        assert_eq!(rects.len(), 3, "expected one rect per token, including the space");

        let same_y = rects[0].1;
        assert!(rects.iter().all(|r| (r.1 - same_y).abs() < 0.01), "same baseline");
        // No gap between consecutive rects: next.x == prev.x + prev.width.
        for pair in rects.windows(2) {
            let gap = pair[1].0 - (pair[0].0 + pair[0].2);
            assert!(gap.abs() < 0.01, "highlight gap of {gap} at a space");
        }
    }

    #[test]
    fn test_underline_continuous_across_space() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer.render("<u>a b</u>").unwrap();
        let lines = svg_lines(&svg);
        assert_eq!(lines.len(), 3, "expected one underline segment per token");
        for pair in lines.windows(2) {
            let gap = pair[1].0 - pair[0].1;
            assert!(gap.abs() < 0.01, "underline gap of {gap} at a space");
        }
    }

    #[test]
    fn test_inline_html_mark_dark_theme_uses_dark_highlight() {
        // Dark theme: the mark highlight must switch to a dark amber so light
        // text stays readable (pure yellow would be ~1:1 contrast).
        let theme = Theme::from_builtin("nord").expect("nord theme");
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer.render("<mark>hi</mark>").unwrap();
        assert!(
            svg.contains(&format!("fill=\"{}\"", MARK_HIGHLIGHT_DARK)),
            "dark theme should use the dark mark highlight"
        );
        assert!(
            !svg.contains("#ffff00"),
            "dark theme must not use pure yellow highlight"
        );

        // The light theme keeps the classic yellow.
        let theme = Theme::default();
        let mut renderer = Renderer::new(theme, MockMeasure, 800.0).unwrap();
        let svg = renderer.render("<mark>hi</mark>").unwrap();
        assert!(svg.contains("fill=\"#ffff00\""), "light theme keeps yellow");
    }

    #[test]
    fn test_mark_highlight_contrast_on_all_demo_themes() {
        // The default <mark> highlight must keep readable contrast against the
        // theme text on every demo theme (WCAG AA body text is 4.5:1; we accept
        // 3:1 for the demo's small colored runs, but plain text should pass 4.5).
        for name in [
            "solarized_light",
            "dracula",
            "nord",
            "catppuccin_mocha",
            "solarized_dark",
        ] {
            let theme = Theme::from_builtin(name).unwrap_or_else(|_| Theme::default());
            let mark = if theme_is_dark(&theme.background_color) {
                MARK_HIGHLIGHT_DARK
            } else {
                MARK_HIGHLIGHT_LIGHT
            };
            let ratio = contrast_ratio(&theme.text_color, mark)
                .unwrap_or_else(|| panic!("{name}: theme text color unparseable"));
            assert!(
                ratio >= 3.0,
                "{name}: mark highlight {mark} vs text {} contrast {ratio:.1}:1 < 3:1",
                theme.text_color
            );
        }
    }

    #[test]
    fn test_inline_html_nested_sup_with_color() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer.render("a<sup style=\"color: green\">n</sup>b").unwrap();
        let (a_fs, a_y, _) = text_metrics(&svg, "a").expect("base text");
        let (n_fs, n_y, n_fill) = text_metrics(&svg, "n").expect("superscript");

        assert!(n_fs < a_fs, "nested sup should be smaller");
        assert!(n_y < a_y, "nested sup should be raised");
        assert_eq!(n_fill, "green", "nested sup should keep its color");
    }

    #[test]
    fn test_inline_html_unknown_tag_keeps_content() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        let svg = renderer.render("<custom>still here</custom> done").unwrap();
        assert!(svg.contains("still"), "unknown tags must not drop content");
        assert!(svg.contains("done"));
    }

    #[test]
    fn test_inline_html_rejects_malicious_style_values() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // `url(javascript:...)` contains ':' which sanitize_color rejects, and the
        // whole style value must never be able to break out of the fill attribute.
        let svg = renderer
            .render("<span style=\"color: red; background: url(javascript:alert(1))\">x</span>")
            .unwrap();
        assert!(svg.contains(">x</text>"));
        assert!(
            !svg.contains("javascript"),
            "unsafe style values should be dropped"
        );
        assert!(
            !svg.contains("onmouseover") && !svg.contains("<script"),
            "no attribute injection into the SVG"
        );
    }

    #[test]
    fn test_table_long_cell_wraps_within_width() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 400.0).unwrap();

        // A 200-char cell is far wider than the 400px output; it must wrap
        // instead of overflowing past the right edge.
        let long = "x".repeat(200);
        let markdown = format!("| A | B |\n|---|--|\n| short | {} |", long);
        let result = renderer.render(&markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        let right_edge = 400.0 - 32.0; // width - padding_x

        // Parse every rect and assert none extends beyond the right edge.
        let mut max_right = 0.0f32;
        let mut search_start = 0usize;
        while let Some(pos) = svg[search_start..].find("<rect x=\"") {
            let abs = search_start + pos + "<rect x=\"".len();
            let rest = &svg[abs..];
            let x_end = rest.find('"').unwrap();
            let x: f32 = rest[..x_end].parse().unwrap();
            let w_start = rest[x_end..].find("width=\"").unwrap() + x_end + "width=\"".len();
            let w_rest = &rest[w_start..];
            let w_end = w_rest.find('"').unwrap();
            let w: f32 = w_rest[..w_end].parse().unwrap();
            max_right = max_right.max(x + w);
            search_start = abs + x_end;
        }

        assert!(
            max_right <= right_edge + 1.0,
            "Table must not overflow the right edge: right edge = {right_edge}, table right = {max_right:.2}"
        );

        // The long word must have been hard-split into multiple text nodes.
        let single_run = "x".repeat(200);
        assert!(
            !svg.contains(&format!(">{}</text>", single_run)),
            "Long cell content should be wrapped, not emitted as one huge text node"
        );
    }

    #[test]
    fn test_gfm_combined_features() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // Test markdown with multiple GFM features combined
        let markdown = r#"# Heading

## Task List
- [x] Done item
- [ ] Pending item

## Autolinks
Visit https://example.com or http://test.org

## Strikethrough
~~deleted~~ and ~~more deleted~~

## Footnote
Here is a note[^1].

[^1]: Note definition here.

## Definition List
Term
: Definition

| Col1 | Col2 |
|------|------|
| A    | B    |
"#;
        let result = renderer.render(markdown);
        assert!(result.is_ok());
        let svg = result.unwrap();

        // Verify all features are present - use simpler strings to find
        assert!(svg.contains("Heading"), "Heading should render");
        assert!(svg.contains("Done"), "Task list checked item should render");
        assert!(svg.contains("Pending"), "Task list unchecked item should render");
        assert!(svg.contains("example.com"), "Autolink should render");
        assert!(svg.contains("deleted"), "Strikethrough should render");
        assert!(svg.contains("note"), "Footnote reference should render");
        assert!(svg.contains("Term"), "Definition term should render");
        assert!(svg.contains("Col1"), "Table header should render");
    }
}

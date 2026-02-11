use crate::fonts::TextMeasure;
use crate::theme::Theme;
use base64::Engine;
use imagesize;
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use resvg::usvg;
use std::path::{Path, PathBuf};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

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

    #[test]
    fn test_renderer_initialization() {
        let theme = Theme::default();
        let measure = MockMeasure;
        let renderer = Renderer::new(theme, measure, 800.0);
        assert!(renderer.is_ok());
    }

    #[test]
    fn test_inline_code_rendering() {
        let mut theme = Theme::default();
        theme.code_padding_y = 10.0;
        theme.font_size_code = 14.0;

        let measure = MockMeasure;
        let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

        // This should trigger render_inline_code
        let markdown = "`code`";
        let result = renderer.render(markdown);

        assert!(result.is_ok());
        let svg = result.unwrap();

        // Check if rect height is calculated correctly according to the new logic
        // rect_height = font_size_code + code_padding_y
        // 14.0 + 10.0 = 24.0
        assert!(svg.contains("height=\"24.00\""));
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
}

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

    in_html_block: bool,
    html_block_buffer: String,

    in_metadata_block: bool,

    definition_list_stack: Vec<DefinitionListState>,

    in_footnote_definition: bool,

    pending_text: String,

    last_margin_added: f32,

    ps: SyntaxSet,
    ts: ThemeSet,

    base_path: Option<PathBuf>,
}

impl<T: TextMeasure> Renderer<T> {
    #[allow(dead_code)]
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
            svg_content: String::new(),
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
            in_html_block: false,
            html_block_buffer: String::new(),
            in_metadata_block: false,
            definition_list_stack: Vec::new(),
            in_footnote_definition: false,
            pending_text: String::new(),
            last_margin_added: 0.0,
            ps,
            ts,
            base_path,
        })
    }

    pub fn render(&mut self, markdown: &str) -> Result<String, String> {
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

        for event in parser {
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
                        self.html_block_buffer.clear();
                        self.in_html_block = false;
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
                self.start_block(0.0, false);
                self.in_code_block = true;
                self.code_block_buffer.clear();
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
                self.start_block(0.0, false);
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
                self.definition_list_stack.push(DefinitionListState { indent });
            }
            Tag::DefinitionListTitle => {
                if !self.at_line_start {
                    self.new_line();
                }
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
                if !self.at_line_start {
                    self.new_line();
                }
            }
            TagEnd::DefinitionListDefinition => {
                self.item_continuation_indent = None;
                if !self.at_line_start {
                    self.new_line();
                }
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

    fn render_token(&mut self, token: &str, is_whitespace: bool) -> Result<(), String> {
        if token.is_empty() {
            return Ok(());
        }

        let font_size = self.current_font_size();
        let is_bold = self.is_bold();
        let is_italic = self.is_italic();

        if is_whitespace {
            if self.at_line_start {
                return Ok(());
            }

            let (space_width, _) = self
                .measure
                .measure_text(" ", font_size, false, is_bold, is_italic, None);

            if self.cursor_x + space_width > self.right_edge() {
                self.advance_line(font_size);
            } else {
                self.cursor_x += space_width;
            }

            return Ok(());
        }

        let (token_width, _) = self
            .measure
            .measure_text(token, font_size, false, is_bold, is_italic, None);

        if !self.at_line_start && self.cursor_x + token_width > self.right_edge() {
            self.advance_line(font_size);
        }

        let fill = self.current_fill().to_string();
        if self.pending_list_marker.is_some() && !self.at_line_start {
            if let Some(pending) = self.pending_list_marker.take() {
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
        }

        self.draw_text_at(
            self.cursor_x,
            self.cursor_y,
            token,
            "sans-serif",
            font_size,
            &fill,
            is_bold,
            is_italic,
        );

        if self.in_strikethrough {
            let line_y = self.cursor_y - font_size * 0.32;
            self.svg_content.push_str(&format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
                self.cursor_x,
                line_y,
                self.cursor_x + token_width,
                line_y,
                fill,
            ));
        }

        if self.link_depth > 0 {
            let underline_y = self.cursor_y + font_size * 0.12;
            self.svg_content.push_str(&format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
                self.cursor_x,
                underline_y,
                self.cursor_x + token_width,
                underline_y,
                fill,
            ));
        }

        self.cursor_x += token_width;
        self.at_line_start = false;

        Ok(())
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
        let rect_height = self.theme.font_size_code + self.theme.code_padding_y;
        // Align roughly to baseline - ascent, then split padding top/bottom
        let rect_y =
            self.cursor_y - self.theme.font_size_code * 0.8 - self.theme.code_padding_y / 2.0;

        self.svg_content.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" />"#,
            self.cursor_x,
            rect_y,
            total_width,
            rect_height,
            self.theme.code_radius,
            self.theme.code_bg_color,
        ));

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

        let (_, _) =
            self.measure
                .measure_text(" ", self.current_font_size(), false, false, false, None);
        self.cursor_x += total_width;
        self.at_line_start = false;

        Ok(())
    }

    fn render_inline_html(&mut self, html: &str) -> Result<(), String> {
        let tag = html.trim().to_ascii_lowercase();

        match tag.as_str() {
            "<br>" | "<br/>" | "<br />" => return self.render_newline(),
            "<del>" => {
                self.in_strikethrough = true;
                return Ok(());
            }
            "</del>" => {
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
            _ => Ok(()),
        }
    }

    fn render_footnote_reference(&mut self, label: &str) -> Result<(), String> {
        let font_size = self.current_font_size();
        let marker = format!("{}", label);
        let superscript_size = font_size * 0.65;
        let (marker_width, _) = self.measure.measure_text(
            &marker,
            superscript_size,
            false,
            false,
            false,
            None,
        );

        if !self.at_line_start && self.cursor_x + marker_width > self.right_edge() {
            self.advance_line(font_size);
        }

        let y = self.cursor_y - font_size * 0.45;
        let fill = self.current_fill().to_string();
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
        let color = self.current_fill().to_string();

        match crate::math::render_math(math_src, font_size, &color, &mut self.measure, false) {
            Ok(result) => {
                if !self.at_line_start && self.cursor_x + result.width > self.right_edge() {
                    self.new_line();
                }
                let rendered = crate::math::render_math_at(
                    math_src, font_size, &color, &mut self.measure, false,
                    self.cursor_x, self.cursor_y,
                ).map_err(|e| format!("Math render error: {}", e))?;
                self.svg_content.push_str(&rendered.svg_fragment);
                self.cursor_x += rendered.width;
                self.at_line_start = false;
                self.last_margin_added = 0.0;
            }
            Err(_) => {
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
        let color = self.current_fill().to_string();

        match crate::math::render_math(&math_src, font_size, &color, &mut self.measure, true) {
            Ok(result) => {
                self.start_block(self.theme.margin_top, false);

                let available_width = self.right_edge() - self.line_start_x();
                let offset_x = self.line_start_x() + (available_width - result.width).max(0.0) / 2.0;
                let baseline_y = self.cursor_y + result.ascent;

                let rendered = crate::math::render_math_at(
                    &math_src, font_size, &color, &mut self.measure, true,
                    offset_x, baseline_y,
                ).map_err(|e| format!("Math render error: {}", e))?;
                self.svg_content.push_str(&rendered.svg_fragment);
                self.cursor_y = baseline_y + rendered.descent;
                self.cursor_x = self.line_start_x();
                self.at_line_start = true;

                self.finish_block(self.theme.margin_bottom);
            }
            Err(_) => {
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
        let block_height = (lines.len().saturating_sub(1) as f32) * line_height
            + self.theme.font_size_code
            + self.theme.code_padding_y * 2.0;
        let block_width = max_line_width + self.theme.code_padding_x * 2.0;

        self.svg_content.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" />"#,
            x,
            self.cursor_y,
            block_width,
            block_height,
            self.theme.code_radius,
            self.theme.code_bg_color,
        ));

        for (idx, line_segments) in lines.iter().enumerate() {
            let y = self.cursor_y
                + self.theme.code_padding_y
                + self.theme.font_size_code * 0.8
                + idx as f32 * line_height;

            let mut current_x = x + self.theme.code_padding_x;

            for (style, text) in line_segments {
                let fill = format!(
                    "#{:02x}{:02x}{:02x}",
                    style.foreground.r, style.foreground.g, style.foreground.b
                );

                self.draw_text_at(
                    current_x,
                    y,
                    text,
                    "monospace",
                    self.theme.font_size_code,
                    &fill,
                    false,
                    false,
                );

                let (w, _) = self.measure.measure_text(
                    text,
                    self.theme.font_size_code,
                    true,
                    false,
                    false,
                    None,
                );
                current_x += w;
            }
        }

        self.cursor_y += block_height;
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

            for ch in text.chars() {
                let candidate_str = String::from(ch);
                let (ch_width, _) = self.measure.measure_text(
                    &candidate_str,
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
        if let Some(state) = self.list_stack.last() {
            if !state.needs_ascent {
                self.advance_line(font_size);
                return Ok(());
            }
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

        self.svg_content.push_str(&format!(
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" />"#,
            left, hr_y, right, hr_y, self.theme.quote_border_color,
        ));

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
        }
    }

    fn finish_table_head(&mut self) {
        if let Some(state) = self.table_state.as_mut() {
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

    fn finish_table_row(&mut self) {
        if let Some(state) = self.table_state.as_mut() {
            if let Some(row) = state.current_row.take() {
                state.rows.push(row);
            }
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
        if let Some(state) = self.table_state.as_mut() {
            if let Some(cell) = state.current_cell.take() {
                if let Some(row) = state.current_row.as_mut() {
                    row.cells.push(cell);
                }
            }
        }
    }

    fn render_table_text(&mut self, text: &str) {
        if let Some(state) = self.table_state.as_mut() {
            if let Some(cell) = state.current_cell.as_mut() {
                cell.text.push_str(text);
            }
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

        let mut column_widths: Vec<f32> = vec![0.0; column_count];
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
                column_widths[idx] = column_widths[idx].max(width as f32);
            }
        }

        let cell_padding_x = self.theme.font_size_base * 0.5;
        let cell_padding_y = self.theme.font_size_base * 0.35;
        let border_color = self.theme.quote_border_color.clone();
        let row_height = self.theme.font_size_base * self.theme.line_height + cell_padding_y * 2.0;
        let table_x = self.line_start_x();
        let table_width: f32 = column_widths.iter().map(|w| w + cell_padding_x * 2.0).sum();

        let mut current_y = self.cursor_y;
        let table_height = row_height * state.rows.len() as f32;

        self.svg_content.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="none" stroke="{}" stroke-width="1" />"#,
            table_x,
            current_y,
            table_width,
            table_height,
            border_color,
        ));

        for row in &state.rows {
            let mut cell_x = table_x;
            for (idx, cell) in row.cells.iter().enumerate() {
                let cell_width = column_widths[idx] + cell_padding_x * 2.0;
                let align = state
                    .alignments
                    .get(idx)
                    .copied()
                    .unwrap_or(Alignment::Left);

                self.svg_content.push_str(&format!(
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="none" stroke="{}" stroke-width="1" />"#,
                    cell_x,
                    current_y,
                    cell_width,
                    row_height,
                    border_color,
                ));

                let (text_width, _) = self.measure.measure_text(
                    cell.text.trim(),
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

                let text_y = current_y + cell_padding_y + self.theme.font_size_base * 0.8;
                let fill = self.current_fill().to_string();
                self.draw_text_at(
                    text_x,
                    text_y,
                    cell.text.trim(),
                    "sans-serif",
                    self.theme.font_size_base,
                    &fill,
                    row.is_header,
                    false,
                );

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
        self.svg_content.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" ry="2" stroke="{}" fill="none" stroke-width="1" />"#,
            x,
            y,
            size,
            size,
            self.current_fill(),
        ));

        if checked {
            let inset = size * 0.2;
            let x1 = x + inset;
            let y1 = y + size * 0.55;
            let x2 = x + size * 0.45;
            let y2 = y + size - inset;
            let x3 = x + size - inset;
            let y3 = y + inset;
            self.svg_content.push_str(&format!(
                r#"<polyline points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="none" stroke="{}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />"#,
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                self.current_fill(),
            ));
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

        let Some(payload) = self.load_image_payload(src)? else {
            return Ok(());
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

        self.svg_content.push_str(&format!(
            r#"<image x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" href="{}" />"#,
            x, y, width, height, payload.data_url,
        ));

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
            let mut response = ureq::get(src)
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

            let bytes = response
                .body_mut()
                .read_to_vec()
                .map_err(|e| format!("Failed to read image {}: {}", src, e))?;

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
            return Ok((size.width() as f32, size.height() as f32));
        }

        let size = imagesize::blob_size(bytes)
            .map_err(|e| format!("Failed to read image size: {}", e))?;
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
        if src_path.is_absolute() {
            return Some(src_path.to_path_buf());
        }

        if let Some(base) = self.base_path.as_ref() {
            return Some(base.join(src));
        }

        Some(src_path.to_path_buf())
    }

    fn start_list_item(&mut self) -> Result<(), String> {
        if self.at_line_start {
            // Move from list block top to first list-item baseline.
            if let Some(state) = self.list_stack.last_mut() {
                if state.needs_ascent {
                    self.cursor_y += self.theme.font_size_base * 0.8;
                    state.needs_ascent = false;
                }
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
        let start_y = self.cursor_y - self.theme.font_size_base * 0.8;

        self.blockquotes.push(QuoteState { border_x, start_y });
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn end_blockquote(&mut self) {
        if !self.at_line_start {
            self.new_line();
        }

        if let Some(quote) = self.blockquotes.pop() {
            self.svg_content.push_str(&format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2" />"#,
                quote.border_x,
                quote.start_y,
                quote.border_x,
                self.cursor_y,
                self.theme.quote_border_color,
            ));
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
        self.cursor_y += font_size * self.current_line_height();
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn current_line_height(&self) -> f32 {
        if self.heading_level.is_some() {
            // Tighter line height for headings
            1.25
        } else {
            self.theme.line_height
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

    fn current_fill(&self) -> &str {
        if self.link_depth > 0 {
            &self.theme.link_color
        } else if self.heading_level.is_some() {
            &self.theme.heading_color
        } else if !self.blockquotes.is_empty() {
            &self.theme.quote_text_color
        } else {
            &self.theme.text_color
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
        let depth_offset = self.list_stack.len().saturating_sub(1) as f32
            * self.theme.font_size_base
            * LIST_INDENT_RATIO;
        self.base_left_indent() + depth_offset
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
    ) {
        self.last_margin_added = 0.0;

        let weight_attr = if bold { " font-weight=\"700\"" } else { "" };
        let style_attr = if italic { " font-style=\"italic\"" } else { "" };

        self.svg_content.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-family="{}" font-size="{:.2}" fill="{}"{}{}>{}</text>"#,
            x,
            y,
            font_family,
            font_size,
            fill,
            weight_attr,
            style_attr,
            self.escape_xml(text).replace(' ', "&#160;"),
        ));
    }

    fn escape_xml(&self, text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    fn finalize_svg(&self, height: f32) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}"><rect width="100%" height="100%" fill="{}" />{}</svg>"#,
            self.width, height, self.width, height, self.theme.background_color, self.svg_content,
        )
    }
}

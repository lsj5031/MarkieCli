use crate::fonts::TextMeasure;
use crate::theme::Theme;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

const LIST_INDENT: f32 = 24.0;
const LIST_MARKER_GAP: f32 = 8.0;
const QUOTE_INDENT: f32 = 20.0;
const QUOTE_INNER_PADDING: f32 = 12.0;

struct ListState {
    ordered: bool,
    next_index: usize,
}

struct QuoteState {
    border_x: f32,
    start_y: f32,
}

pub struct Renderer {
    theme: Theme,
    measure: TextMeasure,
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

    in_code_block: bool,
    code_block_buffer: String,
}

impl Renderer {
    pub fn new(theme: Theme, width: f32) -> Result<Self, String> {
        let measure = TextMeasure::new()?;
        let padding_x = theme.padding_x;
        let padding_y = theme.padding_y;

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
            in_code_block: false,
            code_block_buffer: String::new(),
        })
    }

    pub fn render(&mut self, markdown: &str) -> Result<String, String> {
        let parser = Parser::new(markdown);

        for event in parser {
            if self.in_code_block {
                match event {
                    Event::End(TagEnd::CodeBlock) => {
                        self.render_code_block()?;
                        self.code_block_buffer.clear();
                        self.in_code_block = false;
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

            match event {
                Event::Start(tag) => self.handle_start_tag(tag)?,
                Event::End(tag_end) => self.handle_end_tag(tag_end)?,
                Event::Text(text) => self.render_text(&text)?,
                Event::Code(code) => self.render_inline_code(&code)?,
                Event::SoftBreak | Event::HardBreak => self.render_newline()?,
                Event::Rule => self.render_horizontal_rule()?,
                _ => {}
            }
        }

        let total_height = self.cursor_y + self.theme.padding_y;
        Ok(self.finalize_svg(total_height))
    }

    fn handle_start_tag(&mut self, tag: Tag) -> Result<(), String> {
        match tag {
            Tag::Heading { level, .. } => {
                self.start_block(self.theme.margin_top);
                self.heading_level = Some(level);
            }
            Tag::Paragraph => {
                self.start_block(self.theme.margin_top);
            }
            Tag::CodeBlock(_kind) => {
                self.start_block(self.theme.margin_top);
                self.in_code_block = true;
                self.code_block_buffer.clear();
            }
            Tag::List(start) => {
                if self.list_stack.is_empty() {
                    self.start_block(self.theme.margin_top);
                } else if !self.at_line_start {
                    self.new_line();
                }

                self.list_stack.push(ListState {
                    ordered: start.is_some(),
                    next_index: start.unwrap_or(1) as usize,
                });
            }
            Tag::Item => self.start_list_item()?,
            Tag::BlockQuote(_) => {
                self.start_block(self.theme.margin_top);
                self.start_blockquote();
            }
            Tag::Link { .. } => self.link_depth += 1,
            Tag::Emphasis => self.emphasis_depth += 1,
            Tag::Strong => self.strong_depth += 1,
            _ => {}
        }

        Ok(())
    }

    fn handle_end_tag(&mut self, tag_end: TagEnd) -> Result<(), String> {
        match tag_end {
            TagEnd::Heading(_) => {
                self.heading_level = None;
                self.finish_block(self.theme.margin_bottom);
            }
            TagEnd::Paragraph => {
                self.finish_block(self.theme.margin_bottom);
            }
            TagEnd::CodeBlock => {}
            TagEnd::Item => self.end_list_item(),
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.add_margin(self.theme.margin_bottom);
                    self.cursor_x = self.line_start_x();
                    self.at_line_start = true;
                }
            }
            TagEnd::BlockQuote(_) => {
                self.end_blockquote();
                self.add_margin(self.theme.margin_bottom);
                self.cursor_x = self.line_start_x();
                self.at_line_start = true;
            }
            TagEnd::Link => self.link_depth = self.link_depth.saturating_sub(1),
            TagEnd::Emphasis => self.emphasis_depth = self.emphasis_depth.saturating_sub(1),
            TagEnd::Strong => self.strong_depth = self.strong_depth.saturating_sub(1),
            _ => {}
        }

        Ok(())
    }

    fn render_text(&mut self, text: &str) -> Result<(), String> {
        if text.is_empty() {
            return Ok(());
        }

        let mut start = 0;
        let mut chars = text.char_indices();
        let Some((_, first_ch)) = chars.next() else {
            return Ok(());
        };
        let mut in_whitespace = first_ch.is_whitespace();

        for (idx, ch) in chars {
            if ch.is_whitespace() != in_whitespace {
                let token = &text[start..idx];
                self.render_token(token, in_whitespace)?;
                start = idx;
                in_whitespace = ch.is_whitespace();
            }
        }

        let token = &text[start..];
        self.render_token(token, in_whitespace)
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

        self.cursor_x += token_width;
        self.at_line_start = false;

        Ok(())
    }

    fn render_inline_code(&mut self, code: &str) -> Result<(), String> {
        let (text_width, text_height) =
            self.measure
                .measure_text(code, self.theme.font_size_code, true, false, false, None);

        let total_width = text_width + self.theme.code_padding_x * 2.0;
        if !self.at_line_start && self.cursor_x + total_width > self.right_edge() {
            self.new_line();
        }

        self.svg_content.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" />"#,
            self.cursor_x,
            self.cursor_y - text_height + 4.0,
            total_width,
            text_height + self.theme.code_padding_y * 2.0,
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

        let (space_width, _) =
            self.measure
                .measure_text(" ", self.current_font_size(), false, false, false, None);
        self.cursor_x += total_width + space_width;
        self.at_line_start = false;

        Ok(())
    }

    fn render_code_block(&mut self) -> Result<(), String> {
        let x = self.line_start_x();
        let max_content_width = (self.right_edge() - x - self.theme.code_padding_x * 2.0)
            .max(self.theme.font_size_code);

        let mut lines = Vec::new();
        let code_buffer = self.code_block_buffer.clone();
        for line in code_buffer.lines() {
            self.wrap_code_line(line, max_content_width, &mut lines);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }

        let mut max_line_width: f32 = 0.0;
        for line in &lines {
            let (line_width, _) = self.measure.measure_text(
                line,
                self.theme.font_size_code,
                true,
                false,
                false,
                None,
            );
            max_line_width = max_line_width.max(line_width);
        }

        let line_height = self.theme.font_size_code * self.theme.line_height;
        let block_height = lines.len() as f32 * line_height + self.theme.code_padding_y * 2.0;
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

        let code_text_color = self.theme.code_text_color.clone();
        for (idx, line) in lines.iter().enumerate() {
            let y = self.cursor_y
                + self.theme.code_padding_y
                + self.theme.font_size_code
                + idx as f32 * line_height;

            self.draw_text_at(
                x + self.theme.code_padding_x,
                y,
                line,
                "monospace",
                self.theme.font_size_code,
                &code_text_color,
                false,
                false,
            );
        }

        self.cursor_y += block_height;
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;

        Ok(())
    }

    fn wrap_code_line(&mut self, line: &str, max_width: f32, out: &mut Vec<String>) {
        if line.is_empty() {
            out.push(String::new());
            return;
        }

        let mut current = String::new();
        for ch in line.chars() {
            let mut candidate = current.clone();
            candidate.push(ch);

            let (candidate_width, _) = self.measure.measure_text(
                &candidate,
                self.theme.font_size_code,
                true,
                false,
                false,
                None,
            );

            if candidate_width > max_width && !current.is_empty() {
                out.push(current);
                current = ch.to_string();
            } else {
                current.push(ch);
            }
        }

        out.push(current);
    }

    fn render_newline(&mut self) -> Result<(), String> {
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

        self.add_margin(self.theme.margin_bottom * 0.5);
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
        Ok(())
    }

    fn start_list_item(&mut self) -> Result<(), String> {
        if !self.at_line_start {
            self.new_line();
        }

        let marker = self.next_list_marker();
        let marker_x = self.list_marker_x();
        let (marker_width, _) = self.measure.measure_text(
            &marker,
            self.theme.font_size_base,
            false,
            false,
            false,
            None,
        );

        let fill = self.current_fill().to_string();
        self.draw_text_at(
            marker_x,
            self.cursor_y,
            &marker,
            "sans-serif",
            self.theme.font_size_base,
            &fill,
            false,
            false,
        );

        self.item_continuation_indent = Some(marker_x + marker_width + LIST_MARKER_GAP);
        self.cursor_x = self.item_continuation_indent.unwrap_or(self.line_start_x());
        self.at_line_start = true;

        Ok(())
    }

    fn end_list_item(&mut self) {
        self.item_continuation_indent = None;
        if !self.at_line_start {
            self.new_line();
        }
    }

    fn start_blockquote(&mut self) {
        let depth = self.blockquotes.len() as f32;
        let border_x = self.theme.padding_x + depth * QUOTE_INDENT + QUOTE_INNER_PADDING * 0.5;
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

    fn start_block(&mut self, margin_top: f32) {
        if !self.svg_content.is_empty() {
            if !self.at_line_start {
                self.new_line();
            }
            self.add_margin(margin_top);
        }

        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn finish_block(&mut self, margin_bottom: f32) {
        if !self.at_line_start {
            self.new_line();
        }

        self.add_margin(margin_bottom);
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn add_margin(&mut self, margin: f32) {
        self.cursor_y += margin;
    }

    fn new_line(&mut self) {
        self.advance_line(self.current_font_size());
    }

    fn advance_line(&mut self, font_size: f32) {
        self.cursor_y += font_size * self.theme.line_height;
        self.cursor_x = self.line_start_x();
        self.at_line_start = true;
    }

    fn current_font_size(&self) -> f32 {
        match self.heading_level {
            Some(HeadingLevel::H1) => self.theme.font_size_base * 2.0,
            Some(HeadingLevel::H2) => self.theme.font_size_base * 1.6,
            Some(HeadingLevel::H3) => self.theme.font_size_base * 1.35,
            Some(HeadingLevel::H4) => self.theme.font_size_base * 1.2,
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
        let depth_offset = self.list_stack.len().saturating_sub(1) as f32 * LIST_INDENT;
        self.base_left_indent() + depth_offset
    }

    fn base_left_indent(&self) -> f32 {
        if self.blockquotes.is_empty() {
            self.theme.padding_x
        } else {
            self.theme.padding_x
                + self.blockquotes.len() as f32 * QUOTE_INDENT
                + QUOTE_INNER_PADDING
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
            self.escape_xml(text),
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

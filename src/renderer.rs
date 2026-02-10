use crate::fonts::TextMeasure;
use crate::theme::Theme;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::fonts::TextMeasure;

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
}

struct QuoteState {
    border_x: f32,
    start_y: f32,
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

    in_code_block: bool,
    code_block_buffer: String,
    code_block_lang: Option<String>,
    
    last_margin_added: f32,

    ps: SyntaxSet,
    ts: ThemeSet,
}

impl<T: TextMeasure> Renderer<T> {
    pub fn new(theme: Theme, measure: T, width: f32) -> Result<Self, String> {
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
            in_code_block: false,
            code_block_buffer: String::new(),
            code_block_lang: None,
            last_margin_added: 0.0,
            ps,
            ts,
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

        let total_height = self.cursor_y + self.theme.padding_y - self.last_margin_added;
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
                self.start_block(self.theme.margin_top * top_margin_scale);
            }
            Tag::Paragraph => {
                self.start_block(0.0);
            }
            Tag::CodeBlock(kind) => {
                self.start_block(0.0);
                self.in_code_block = true;
                self.code_block_buffer.clear();
                self.code_block_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                    _ => None,
                };
            }
            Tag::List(start) => {
                if self.list_stack.is_empty() {
                    self.start_block(0.0);
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
                self.start_block(0.0);
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
                let bottom_margin_scale = match self.heading_level {
                    Some(HeadingLevel::H1) | Some(HeadingLevel::H2) => 0.65,
                    _ => 1.0,
                };
                self.finish_block(self.theme.margin_bottom * bottom_margin_scale);
                self.heading_level = None;
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
        let (text_width, _text_height) =
            self.measure
                .measure_text(code, self.theme.font_size_code, true, false, false, None);

        let total_width = text_width + self.theme.code_padding_x * 2.0;
        if !self.at_line_start && self.cursor_x + total_width > self.right_edge() {
            self.new_line();
        }

        // Tighter background box based on font size
        let rect_height = self.theme.font_size_code + self.theme.code_padding_y;
        // Align roughly to baseline - ascent + padding
        let rect_y = self.cursor_y - self.theme.font_size_code * 0.8;

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

    fn render_code_block(&mut self) -> Result<(), String> {
        let x = self.line_start_x();
        let max_content_width = (self.right_edge() - x - self.theme.code_padding_x * 2.0)
            .max(self.theme.font_size_code);

        // 1. Highlight Phase
        let mut raw_highlighted_lines: Vec<Vec<(SyntectStyle, String)>> = Vec::new();
        let code_buffer = self.code_block_buffer.clone();

        {
            let lang = self.code_block_lang.as_deref().unwrap_or("txt");
            let syntax = self.ps.find_syntax_by_token(lang)
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

            let theme_name = if is_dark { "Solarized (dark)" } else { "Solarized (light)" };
            let theme = self.ts.themes.get(theme_name)
                .or_else(|| self.ts.themes.get(if is_dark { "base16-ocean.dark" } else { "base16-ocean.light" }))
                .unwrap_or_else(|| self.ts.themes.values().next().unwrap());

            let mut highlighter = HighlightLines::new(syntax, theme);
            
            for line in code_buffer.lines() {
                let ranges = highlighter.highlight_line(line, &self.ps)
                    .map_err(|e| format!("Highlight error: {}", e))?;
                
                raw_highlighted_lines.push(ranges.iter().map(|(s, t)| (*s, t.to_string())).collect());
            }
        }

        // 2. Wrap Phase
        let mut lines = Vec::new();
        for line_segments in raw_highlighted_lines {
            let segments_ref: Vec<(SyntectStyle, &str)> = line_segments.iter()
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

        for (idx, line_segments) in lines.iter().enumerate() {
            let y = self.cursor_y
                + self.theme.code_padding_y
                + self.theme.font_size_code
                + idx as f32 * line_height;

            let mut current_x = x + self.theme.code_padding_x;

            for (style, text) in line_segments {
                let fill = format!("#{:02x}{:02x}{:02x}", style.foreground.r, style.foreground.g, style.foreground.b);
                
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

    fn wrap_styled_line(&mut self, segments: &[(SyntectStyle, &str)], max_width: f32, out: &mut Vec<Vec<(SyntectStyle, String)>>) {
        if segments.is_empty() {
             out.push(Vec::new());
             return;
        }

        let mut current_line: Vec<(SyntectStyle, String)> = Vec::new();
        let mut current_line_width = 0.0;

        for (style, text) in segments {
            if text.is_empty() { continue; }
            
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

        self.item_continuation_indent = Some(marker_x + marker_width + self.theme.font_size_base * LIST_MARKER_GAP_RATIO);
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

    fn start_block(&mut self, margin_top: f32) {
        if !self.svg_content.is_empty() {
            if !self.at_line_start {
                self.new_line();
            }
            self.add_margin(margin_top);
        } else {
            self.cursor_y += self.current_font_size() * 0.8;
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
        let depth_offset = self.list_stack.len().saturating_sub(1) as f32 * self.theme.font_size_base * LIST_INDENT_RATIO;
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

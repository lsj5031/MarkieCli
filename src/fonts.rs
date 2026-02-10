use std::collections::HashMap;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, Weight};

#[derive(Hash, PartialEq, Eq)]
struct MeasureKey {
    text: String,
    font_size_bits: u32,
    is_code: bool,
    is_bold: bool,
    is_italic: bool,
    max_width_bits: Option<u32>,
}

pub struct TextMeasure {
    font_system: FontSystem,
    cache: HashMap<MeasureKey, (f32, f32)>,
}

impl TextMeasure {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            font_system: FontSystem::new(),
            cache: HashMap::new(),
        })
    }

    pub fn measure_text(
        &mut self,
        text: &str,
        font_size: f32,
        is_code: bool,
        is_bold: bool,
        is_italic: bool,
        max_width: Option<f32>,
    ) -> (f32, f32) {
        let key = MeasureKey {
            text: text.to_string(),
            font_size_bits: font_size.to_bits(),
            is_code,
            is_bold,
            is_italic,
            max_width_bits: max_width.map(f32::to_bits),
        };

        if let Some(cached) = self.cache.get(&key) {
            return *cached;
        }

        let line_height = font_size * 1.2;
        let mut buffer = Buffer::new(
            &mut self.font_system,
            Metrics {
                font_size,
                line_height,
            },
        );

        buffer.set_size(
            &mut self.font_system,
            Some(max_width.unwrap_or(f32::MAX)),
            Some(f32::MAX),
        );

        let attrs = Attrs::new()
            .family(if is_code {
                Family::Monospace
            } else {
                Family::SansSerif
            })
            .weight(if is_bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .style(if is_italic {
                Style::Italic
            } else {
                Style::Normal
            });

        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);

        let mut total_width: f32 = 0.0;
        let mut total_height: f32 = 0.0;

        for run in buffer.layout_runs() {
            total_width = total_width.max(run.line_w);
            total_height += run.line_height;
        }

        let measured = (total_width, total_height);
        self.cache.insert(key, measured);
        measured
    }
}

impl Default for TextMeasure {
    fn default() -> Self {
        Self::new().expect("Failed to initialize font system")
    }
}

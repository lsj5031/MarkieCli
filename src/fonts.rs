use std::num::NonZeroUsize;
use std::sync::LazyLock;

use cosmic_text::skrifa::{FontRef, raw::TableProvider, raw::tables::head::Flags};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, Weight, fontdb};
use lru::LruCache;
use parking_lot::Mutex;

#[derive(Hash, PartialEq, Eq, Clone)]
struct MeasureKey {
    text: String,
    font_size_bits: u32,
    is_code: bool,
    is_bold: bool,
    is_italic: bool,
    max_width_bits: Option<u32>,
}

pub trait TextMeasure {
    fn measure_text(
        &mut self,
        text: &str,
        font_size: f32,
        is_code: bool,
        is_bold: bool,
        is_italic: bool,
        max_width: Option<f32>,
    ) -> (f32, f32);
}

/// Global font system with thread-safe LRU cache.
/// This singleton ensures:
/// - Single FontSystem instance (expensive to create)
/// - Shared cache across all measurements
/// - Automatic LRU eviction to manage memory
static GLOBAL_FONT_SYSTEM: LazyLock<GlobalFontSystem> = LazyLock::new(GlobalFontSystem::new);

/// Resolve the actual generic faces used by measurement, including fallback
/// when a system's configured generic family is unavailable.
pub(crate) fn default_font_families() -> (Option<String>, Option<String>) {
    let mut fonts = GLOBAL_FONT_SYSTEM.font_system.lock();
    if fonts.db().faces().next().is_none() {
        return (None, None);
    }
    let mut resolve = |family| {
        let mut buffer = Buffer::new(&mut fonts, Metrics::new(16.0, 19.2));
        buffer.set_text(
            &mut fonts,
            "M",
            &Attrs::new().family(family),
            Shaping::Advanced,
            None,
        );
        let id = buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter())
            .next()?
            .font_id;
        fonts
            .db()
            .face(id)?
            .families
            .first()
            .map(|(name, _)| name.clone())
    };
    (resolve(Family::SansSerif), resolve(Family::Monospace))
}

/// Exclude invalid metrics and last-resort faces that mask real glyph fallbacks.
pub(crate) fn remove_unusable_fonts(db: &mut fontdb::Database) {
    let unusable: Vec<_> = db
        .faces()
        .filter_map(|face| {
            let usable = db
                .with_face_data(face.id, |data, index| {
                    FontRef::from_index(data, index).is_ok_and(|font| {
                        font.head().is_ok_and(|head| {
                            head.units_per_em() > 0
                                && !head.flags().contains(Flags::LAST_RESORT_FONT)
                        })
                    })
                })
                .unwrap_or(false);
            (!usable).then_some(face.id)
        })
        .collect();
    for id in unusable {
        db.remove_face(id);
    }
}

struct GlobalFontSystem {
    font_system: Mutex<FontSystem>,
    cache: Mutex<LruCache<MeasureKey, (f32, f32)>>,
}

impl GlobalFontSystem {
    fn new() -> Self {
        // Cache up to ~10MB of text measurements (estimated ~100 bytes per entry average)
        const CACHE_CAPACITY: usize = 100_000;
        let (locale, mut db) = FontSystem::new().into_locale_and_db();
        let has_system_fonts = db.faces().next().is_some();
        db.load_fonts_dir("fonts");
        remove_unusable_fonts(&mut db);
        // SFNS is variable: measurement and the PDF renderer disagree on bold
        // selection. Helvetica supplies matching static faces for all styles.
        #[cfg(target_os = "macos")]
        if db
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == "Helvetica"))
        {
            db.set_sans_serif_family("Helvetica");
        }
        if !has_system_fonts {
            let sans = db
                .faces()
                .find(|face| !face.monospaced)
                .or_else(|| db.faces().next())
                .and_then(|face| face.families.first().map(|(name, _)| name.clone()));
            let mono = db
                .faces()
                .find(|face| face.monospaced)
                .and_then(|face| face.families.first().map(|(name, _)| name.clone()));
            if let Some(sans) = sans {
                db.set_sans_serif_family(&sans);
                db.set_serif_family(&sans);
                db.set_monospace_family(mono.as_deref().unwrap_or(&sans));
            }
        }
        Self {
            font_system: Mutex::new(FontSystem::new_with_locale_and_db(locale, db)),
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(CACHE_CAPACITY).unwrap())),
        }
    }

    fn measure_text(
        &self,
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

        // Check cache first
        {
            let mut cache = self.cache.lock();
            if let Some(&cached) = cache.get(&key) {
                return cached;
            }
        }

        // Perform measurement with font_system lock
        let measured = {
            let mut font_system = self.font_system.lock();
            measure_text_impl(
                &mut font_system,
                text,
                font_size,
                is_code,
                is_bold,
                is_italic,
                max_width,
            )
        };

        // Store in cache
        {
            let mut cache = self.cache.lock();
            cache.put(key, measured);
        }

        measured
    }
}

fn measure_text_impl(
    font_system: &mut FontSystem,
    text: &str,
    font_size: f32,
    is_code: bool,
    is_bold: bool,
    is_italic: bool,
    max_width: Option<f32>,
) -> (f32, f32) {
    let line_height = font_size * 1.2;
    let mut buffer = Buffer::new(
        font_system,
        Metrics {
            font_size,
            line_height,
        },
    );

    buffer.set_size(font_system, max_width, None);

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

    buffer.set_text(font_system, text, &attrs, Shaping::Advanced, None);

    let mut total_width: f32 = 0.0;
    let mut total_height: f32 = 0.0;

    for run in buffer.layout_runs() {
        total_width = total_width.max(run.line_w);
        total_height += run.line_height;
    }

    (total_width, total_height)
}

/// Text measurement using the global font system with LRU cache.
/// Multiple instances share the same underlying font system and cache.
pub struct CosmicTextMeasure;

impl CosmicTextMeasure {
    pub fn new() -> Result<Self, String> {
        if GLOBAL_FONT_SYSTEM
            .font_system
            .lock()
            .db()
            .faces()
            .next()
            .is_none()
        {
            return Err("No usable fonts found. Install system fonts (for example DejaVu Sans), or place TTF/OTF fonts in ./fonts in the current working directory.".to_string());
        }
        Ok(Self)
    }
}

impl TextMeasure for CosmicTextMeasure {
    fn measure_text(
        &mut self,
        text: &str,
        font_size: f32,
        is_code: bool,
        is_bold: bool,
        is_italic: bool,
        max_width: Option<f32>,
    ) -> (f32, f32) {
        GLOBAL_FONT_SYSTEM.measure_text(text, font_size, is_code, is_bold, is_italic, max_width)
    }
}

impl Default for CosmicTextMeasure {
    fn default() -> Self {
        Self::new().expect("Failed to initialize font system")
    }
}

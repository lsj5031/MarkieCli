use resvg::usvg;
use std::path::Path;
use tiny_skia::{Pixmap, Transform};

pub fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(format!("Invalid --png-scale value: {}", scale));
    }

    let mut opts = usvg::Options::default();
    {
        let fontdb = opts.fontdb_mut();
        fontdb.load_system_fonts();

        let local_fonts = Path::new("fonts");
        if local_fonts.is_dir() {
            fontdb.load_fonts_dir(local_fonts);
        }

        configure_font_fallbacks(fontdb);
    }

    let tree =
        usvg::Tree::from_str(svg, &opts).map_err(|e| format!("Failed to parse SVG: {}", e))?;

    let svg_width = (tree.size().width() * scale).ceil() as u32;
    let svg_height = (tree.size().height() * scale).ceil() as u32;

    let mut pixmap = Pixmap::new(svg_width, svg_height).ok_or("Failed to create pixmap")?;
    let transform = Transform::from_scale(scale, scale);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .map_err(|e| format!("Failed to encode PNG: {}", e))
}

pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>, String> {
    use svg2pdf::usvg::fontdb;

    // Configure font options
    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();

    let local_fonts = Path::new("fonts");
    if local_fonts.is_dir() {
        fontdb.load_fonts_dir(local_fonts);
    }

    configure_font_fallbacks_svg2pdf(&mut fontdb);

    let opts = svg2pdf::usvg::Options {
        fontdb: std::sync::Arc::new(fontdb),
        ..Default::default()
    };

    // Parse the SVG
    let tree = svg2pdf::usvg::Tree::from_str(svg, &opts)
        .map_err(|e| format!("Failed to parse SVG: {}", e))?;

    // Convert to PDF.
    // Keep text as paths for broader viewer/font compatibility.
    // This avoids PDFs with missing text when font embedding fails.
    let options = svg2pdf::ConversionOptions {
        embed_text: false,
        ..Default::default()
    };
    let page_options = svg2pdf::PageOptions::default();

    svg2pdf::to_pdf(&tree, options, page_options)
        .map_err(|e| format!("Failed to convert SVG to PDF: {}", e))
}

pub fn save_output(svg: &str, output: &Path, png_scale: f32) -> Result<(), String> {
    let output_ext = output
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("Output file has no extension")?
        .to_ascii_lowercase();

    match output_ext.as_str() {
        "svg" => {
            std::fs::write(output, svg).map_err(|e| format!("Failed to write SVG: {}", e))?;
            eprintln!("SVG saved to: {}", output.display());
        }
        "png" => {
            let png_data = svg_to_png(svg, png_scale)?;
            std::fs::write(output, png_data).map_err(|e| format!("Failed to write PNG: {}", e))?;
            eprintln!("PNG saved to: {}", output.display());
        }
        "pdf" => {
            let pdf_data = svg_to_pdf(svg)?;
            std::fs::write(output, pdf_data).map_err(|e| format!("Failed to write PDF: {}", e))?;
            eprintln!("PDF saved to: {}", output.display());
        }
        _ => {
            return Err(format!(
                "Unsupported output format: .{} (use .svg, .png or .pdf)",
                output_ext
            ));
        }
    }

    Ok(())
}

/// Configure generic-family fallbacks on a font database. The resvg (PNG) and
/// svg2pdf (PDF) paths pull in different usvg versions, so their
/// `fontdb::Database` types are unrelated and can't be unified through a shared
/// function or trait — hence a macro.
macro_rules! configure_font_fallbacks {
    ($fontdb:expr) => {{
        crate::fonts::remove_unusable_fonts($fontdb);
        let families: Vec<(String, String, bool)> = $fontdb
            .faces()
            .flat_map(|face| {
                face.families.iter().map(move |(family, _)| {
                    (family.clone(), family.to_ascii_lowercase(), face.monospaced)
                })
            })
            .collect();

        let (sans_family, mono_family) = crate::fonts::default_font_families();
        let serif_family = families
            .iter()
            .find(|(_, lower, mono)| lower.contains("serif") && !mono)
            .map(|(family, _, _)| family.clone());
        let first_family = families
            .iter()
            .find(|(_, _, mono)| !mono)
            .or(families.first())
            .map(|(f, _, _)| f.clone());

        if let Some(family) = sans_family.as_deref().or(first_family.as_deref()) {
            $fontdb.set_sans_serif_family(family);
        }
        if let Some(family) = serif_family
            .as_deref()
            .or(sans_family.as_deref())
            .or(first_family.as_deref())
        {
            $fontdb.set_serif_family(family);
        }
        if let Some(family) = mono_family
            .as_deref()
            .or(sans_family.as_deref())
            .or(first_family.as_deref())
        {
            $fontdb.set_monospace_family(family);
        }
    }};
}

fn configure_font_fallbacks(fontdb: &mut usvg::fontdb::Database) {
    configure_font_fallbacks!(fontdb);
}

fn configure_font_fallbacks_svg2pdf(fontdb: &mut svg2pdf::usvg::fontdb::Database) {
    configure_font_fallbacks!(fontdb);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_save_output_missing_extension() {
        let result = save_output("<svg></svg>", Path::new("output"), 1.0);
        assert_eq!(result.unwrap_err(), "Output file has no extension");
    }

    #[test]
    fn test_save_output_unsupported_extension() {
        let result = save_output("<svg></svg>", Path::new("output.txt"), 1.0);
        assert!(
            result
                .unwrap_err()
                .contains("Unsupported output format: .txt")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_text_preserves_spacing_and_glyph_fallback() {
        use crate::{fonts::CosmicTextMeasure, renderer::Renderer, theme::Theme};

        let render = |markdown| {
            let theme = Theme {
                background_color: "#ffffff".into(),
                text_color: "#000000".into(),
                ..Theme::default()
            };
            Renderer::new(theme, CosmicTextMeasure::new().unwrap(), 640.0)
                .unwrap()
                .render(markdown)
                .unwrap()
        };
        let ink_right = |svg: &str| {
            let image = Pixmap::decode_png(&svg_to_png(svg, 1.0).unwrap()).unwrap();
            let background = &image.data()[..4];
            image
                .data()
                .chunks_exact(4)
                .enumerate()
                .filter(|(_, pixel)| *pixel != background)
                .map(|(index, _)| index % image.width() as usize)
                .max()
                .unwrap()
        };
        // NBSP keeps the line in one text run; ordinary spaces split words.
        // Both forms must paint with the same advances, including on SFNS hosts.
        let words = ink_right(&render("Alpha beta gamma."));
        let whole_run = ink_right(&render("Alpha\u{a0}beta\u{a0}gamma."));
        assert!(words.abs_diff(whole_run) <= 2, "{words} vs {whole_run}");

        for markdown in [
            "Before `中文` after.",
            "| Code |\n|---|\n| `中文` |",
            "```text\n中文\n```",
        ] {
            let svg = render(markdown);
            assert!(!svg.contains("inf") && !svg.contains("NaN"), "{svg}");
            svg_to_png(&svg, 1.0).unwrap();
            svg_to_pdf(&svg).unwrap();
        }

        // LastResort boxes have the prose color; a real Apple emoji contains
        // colored pixels distinct from the text and page background.
        // The mixed code case also catches failed fallback merges caused by
        // different glyph counts for combining accents and ZWJ sequences.
        for markdown in ["🧑‍💻", "`🧑‍💻`", "`code  e\u{301}🧑‍💻`"] {
            let svg = render(markdown);
            let image = Pixmap::decode_png(&svg_to_png(&svg, 1.0).unwrap()).unwrap();
            assert!(
                image.data().chunks_exact(4).any(|pixel| {
                    pixel[..3].iter().max().unwrap() - pixel[..3].iter().min().unwrap() > 80
                }),
                "missing color emoji: {markdown}"
            );
            svg_to_pdf(&svg).unwrap();
        }
    }
}

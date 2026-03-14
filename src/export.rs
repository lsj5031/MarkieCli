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
            std::fs::write(output, png_data)
                .map_err(|e| format!("Failed to write PNG: {}", e))?;
            eprintln!("PNG saved to: {}", output.display());
        }
        "pdf" => {
            let pdf_data = svg_to_pdf(svg)?;
            std::fs::write(output, pdf_data)
                .map_err(|e| format!("Failed to write PDF: {}", e))?;
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

fn configure_font_fallbacks(fontdb: &mut usvg::fontdb::Database) {
    let mut sans_family: Option<String> = None;
    let mut serif_family: Option<String> = None;
    let mut mono_family: Option<String> = None;
    let mut first_family: Option<String> = None;

    for face in fontdb.faces() {
        for (family, _) in &face.families {
            if first_family.is_none() {
                first_family = Some(family.clone());
            }

            let lower = family.to_ascii_lowercase();
            if sans_family.is_none() && lower.contains("sans") {
                sans_family = Some(family.clone());
            }
            if serif_family.is_none() && lower.contains("serif") {
                serif_family = Some(family.clone());
            }
            if mono_family.is_none() && (lower.contains("mono") || lower.contains("code")) {
                mono_family = Some(family.clone());
            }
        }
    }

    if let Some(family) = sans_family.as_deref().or(first_family.as_deref()) {
        fontdb.set_sans_serif_family(family);
    }
    if let Some(family) = serif_family.as_deref().or(first_family.as_deref()) {
        fontdb.set_serif_family(family);
    }
    if let Some(family) = mono_family
        .as_deref()
        .or(sans_family.as_deref())
        .or(first_family.as_deref())
    {
        fontdb.set_monospace_family(family);
    }
}

fn configure_font_fallbacks_svg2pdf(fontdb: &mut svg2pdf::usvg::fontdb::Database) {
    let mut sans_family: Option<String> = None;
    let mut serif_family: Option<String> = None;
    let mut mono_family: Option<String> = None;
    let mut first_family: Option<String> = None;

    for face in fontdb.faces() {
        for (family, _) in &face.families {
            if first_family.is_none() {
                first_family = Some(family.clone());
            }

            let lower = family.to_ascii_lowercase();
            if sans_family.is_none() && lower.contains("sans") {
                sans_family = Some(family.clone());
            }
            if serif_family.is_none() && lower.contains("serif") {
                serif_family = Some(family.clone());
            }
            if mono_family.is_none() && (lower.contains("mono") || lower.contains("code")) {
                mono_family = Some(family.clone());
            }
        }
    }

    if let Some(family) = sans_family.as_deref().or(first_family.as_deref()) {
        fontdb.set_sans_serif_family(family);
    }
    if let Some(family) = serif_family.as_deref().or(first_family.as_deref()) {
        fontdb.set_serif_family(family);
    }
    if let Some(family) = mono_family
        .as_deref()
        .or(sans_family.as_deref())
        .or(first_family.as_deref())
    {
        fontdb.set_monospace_family(family);
    }
}

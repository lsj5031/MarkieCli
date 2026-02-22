use clap::Parser;
use markie::fonts::CosmicTextMeasure;
use markie::mermaid::{render_diagram, DiagramStyle};
use markie::theme::Theme;
use resvg::usvg;
use std::path::{Path, PathBuf};
use tiny_skia::{Pixmap, Transform};

/// Standalone Mermaid diagram renderer (SVG/PNG/PDF)
#[derive(Parser, Debug)]
#[command(name = "markie-mermaid")]
#[command(version)]
#[command(about = "Render Mermaid diagrams to SVG, PNG or PDF", long_about = None)]
struct Args {
    /// Input .mmd file (use "-" for stdin)
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output file path (extension determines format: .svg, .png or .pdf)
    #[arg(short, long, value_name = "OUTPUT")]
    output: PathBuf,

    /// Path to Alacritty theme file (YAML or TOML)
    #[arg(short, long, value_name = "THEME")]
    theme: Option<PathBuf>,

    /// Raster scale multiplier for PNG output
    #[arg(long, default_value_t = 1.0)]
    png_scale: f32,

    /// Padding around the diagram in pixels
    #[arg(long, default_value_t = 20.0)]
    padding: f32,
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    let theme = if let Some(ref theme_path) = args.theme {
        if theme_path.exists() && theme_path.is_file() {
            let content = std::fs::read_to_string(theme_path)
                .map_err(|e| format!("Failed to read theme file: {}", e))?;
            if let Ok(theme) = Theme::from_alacritty_toml(&content) {
                theme
            } else if let Ok(theme) = Theme::from_alacritty_yaml(&content) {
                theme
            } else {
                return Err("Failed to parse theme file as TOML or YAML".to_string());
            }
        } else {
            return Err(format!("Theme file not found: {}", theme_path.display()));
        }
    } else {
        Theme::default()
    };

    let source = if args.input.to_str() == Some("-") {
        let mut buffer = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
            .map_err(|e| format!("Failed to read from stdin: {}", e))?;
        buffer
    } else {
        std::fs::read_to_string(&args.input)
            .map_err(|e| format!("Failed to read input file: {}", e))?
    };

    let style = DiagramStyle::from_theme(
        &theme.text_color,
        &theme.background_color,
        &theme.code_bg_color,
    );

    let mut measure = CosmicTextMeasure::new()?;
    let (inner_svg, width, height) = render_diagram(&source, &style, &mut measure)?;

    let pad = args.padding;
    let total_w = width + pad * 2.0;
    let total_h = height + pad * 2.0;

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{total_w}" height="{total_h}" viewBox="0 0 {total_w} {total_h}">
<rect width="{total_w}" height="{total_h}" fill="{canvas_bg}"/>
<g transform="translate({pad},{pad})">
{inner}
</g>
</svg>"#,
        total_w = total_w,
        total_h = total_h,
        // Keep standalone rendering consistent with DiagramStyle contrast decisions.
        // Diagram strokes/text are chosen against code_bg.
        canvas_bg = style.node_fill,
        pad = pad,
        inner = inner_svg,
    );

    let output_ext = args
        .output
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("Output file has no extension")?
        .to_ascii_lowercase();

    match output_ext.as_str() {
        "svg" => {
            std::fs::write(&args.output, &svg)
                .map_err(|e| format!("Failed to write SVG: {}", e))?;
            eprintln!("SVG saved to: {}", args.output.display());
        }
        "png" => {
            let png_data = svg_to_png(&svg, args.png_scale)?;
            std::fs::write(&args.output, png_data)
                .map_err(|e| format!("Failed to write PNG: {}", e))?;
            eprintln!("PNG saved to: {}", args.output.display());
        }
        "pdf" => {
            let pdf_data = svg_to_pdf(&svg)?;
            std::fs::write(&args.output, pdf_data)
                .map_err(|e| format!("Failed to write PDF: {}", e))?;
            eprintln!("PDF saved to: {}", args.output.display());
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

fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
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

fn svg_to_pdf(svg: &str) -> Result<Vec<u8>, String> {
    use svg2pdf::usvg::fontdb;

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

    let tree = svg2pdf::usvg::Tree::from_str(svg, &opts)
        .map_err(|e| format!("Failed to parse SVG: {}", e))?;

    let options = svg2pdf::ConversionOptions {
        embed_text: false,
        ..Default::default()
    };
    let page_options = svg2pdf::PageOptions::default();

    svg2pdf::to_pdf(&tree, options, page_options)
        .map_err(|e| format!("Failed to convert SVG to PDF: {}", e))
}

fn configure_font_fallbacks(fontdb: &mut usvg::fontdb::Database) {
    let mut sans_family: Option<String> = None;
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
            if mono_family.is_none() && (lower.contains("mono") || lower.contains("code")) {
                mono_family = Some(family.clone());
            }
        }
    }

    if let Some(family) = sans_family.as_deref().or(first_family.as_deref()) {
        fontdb.set_sans_serif_family(family);
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
            if mono_family.is_none() && (lower.contains("mono") || lower.contains("code")) {
                mono_family = Some(family.clone());
            }
        }
    }

    if let Some(family) = sans_family.as_deref().or(first_family.as_deref()) {
        fontdb.set_sans_serif_family(family);
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

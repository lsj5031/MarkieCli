mod fonts;
mod math;
mod renderer;
mod theme;

use clap::Parser;
use resvg::usvg;
use std::path::{Path, PathBuf};
use tiny_skia::{Pixmap, Transform};

/// A pure Rust Markdown to SVG/PNG renderer
#[derive(Parser, Debug)]
#[command(name = "markie")]
#[command(about = "Render Markdown to beautiful SVG or PNG images", long_about = None)]
struct Args {
    /// Input markdown file (use "-" for stdin)
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output file path (extension determines format: .svg or .png)
    #[arg(short, long, value_name = "OUTPUT")]
    output: PathBuf,

    /// Path to Alacritty theme file (YAML or TOML)
    #[arg(short, long, value_name = "THEME")]
    theme: Option<PathBuf>,

    /// Image width in pixels
    #[arg(short, long, default_value_t = 800.0)]
    width: f32,
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    // Load theme
    let theme = if let Some(ref theme_path) = args.theme {
        if theme_path.exists() && theme_path.is_file() {
            let content = std::fs::read_to_string(theme_path)
                .map_err(|e| format!("Failed to read theme file: {}", e))?;

            // Try TOML first (since Alacritty is moving to TOML), then YAML
            if let Ok(theme) = theme::Theme::from_alacritty_toml(&content) {
                theme
            } else if let Ok(theme) = theme::Theme::from_alacritty_yaml(&content) {
                theme
            } else {
                return Err("Failed to parse theme file as TOML or YAML".to_string());
            }
        } else {
            return Err(format!("Theme file not found: {}", theme_path.display()));
        }
    } else {
        theme::Theme::default()
    };

    // Read markdown input
    let markdown = if args.input.to_str() == Some("-") {
        let mut buffer = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
            .map_err(|e| format!("Failed to read from stdin: {}", e))?;
        buffer
    } else {
        std::fs::read_to_string(&args.input)
            .map_err(|e| format!("Failed to read input file: {}", e))?
    };

    let base_path = if args.input.to_str() == Some("-") {
        None
    } else {
        args.input.parent().map(|path| path.to_path_buf())
    };

    // Render to SVG
    let measure = fonts::CosmicTextMeasure::new()?;
    let mut renderer =
        renderer::Renderer::new_with_base_path(theme, measure, args.width, base_path)?;
    let svg = renderer.render(&markdown)?;

    // Determine output format and save
    let output_ext = args
        .output
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("Output file has no extension")?
        .to_ascii_lowercase();

    match output_ext.as_str() {
        "svg" => {
            std::fs::write(&args.output, svg).map_err(|e| format!("Failed to write SVG: {}", e))?;
            eprintln!("SVG saved to: {}", args.output.display());
        }
        "png" => {
            let png_data = svg_to_png(&svg)?;
            std::fs::write(&args.output, png_data)
                .map_err(|e| format!("Failed to write PNG: {}", e))?;
            eprintln!("PNG saved to: {}", args.output.display());
        }
        _ => {
            return Err(format!(
                "Unsupported output format: .{} (use .svg or .png)",
                output_ext
            ));
        }
    }

    Ok(())
}

fn svg_to_png(svg: &str) -> Result<Vec<u8>, String> {
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

    let svg_width = tree.size().width() as u32;
    let svg_height = tree.size().height() as u32;

    let mut pixmap = Pixmap::new(svg_width, svg_height).ok_or("Failed to create pixmap")?;
    let transform = Transform::default();

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .map_err(|e| format!("Failed to encode PNG: {}", e))
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

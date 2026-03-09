use clap::Parser;
use markie::fonts::CosmicTextMeasure;
use markie::mermaid::{render_diagram, DiagramStyle};
use markie::theme::Theme;
use std::path::PathBuf;

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
            let png_data = markie::export::svg_to_png(&svg, args.png_scale)?;
            std::fs::write(&args.output, png_data)
                .map_err(|e| format!("Failed to write PNG: {}", e))?;
            eprintln!("PNG saved to: {}", args.output.display());
        }
        "pdf" => {
            let pdf_data = markie::export::svg_to_pdf(&svg)?;
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

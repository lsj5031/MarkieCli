use clap::{CommandFactory, Parser};
use markie::{fonts, renderer, theme};
use std::path::{Path, PathBuf};

/// A pure Rust Markdown to SVG/PNG/PDF renderer
#[derive(Parser, Debug)]
#[command(name = "markie")]
#[command(version)]
#[command(about = "Render Markdown to beautiful SVG, PNG or PDF images", long_about = None)]
struct Args {
    /// Input markdown file (use "-" for stdin)
    #[arg(value_name = "INPUT", required_unless_present_any = ["completions", "list_themes"])]
    input: Option<PathBuf>,

    /// Output file path (extension determines format: .svg, .png or .pdf) [default: INPUT.png]
    #[arg(short, long, value_name = "OUTPUT")]
    output: Option<PathBuf>,

    /// Theme name or path to Alacritty theme file (YAML or TOML)
    #[arg(short, long, value_name = "THEME")]
    theme: Option<String>,

    /// List available built-in themes and exit
    #[arg(long)]
    list_themes: bool,

    /// Image width in pixels
    #[arg(short, long, default_value_t = 1200.0)]
    width: f32,

    /// Raster scale multiplier for PNG output (e.g. 2.0 for sharper output)
    #[arg(long, default_value_t = 2.0)]
    png_scale: f32,

    /// Generate shell completions and exit
    #[arg(long, value_name = "SHELL")]
    completions: Option<clap_complete::Shell>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();

    if let Some(shell) = args.completions {
        let mut cmd = Args::command();
        clap_complete::generate(shell, &mut cmd, "markie", &mut std::io::stdout());
        return Ok(());
    }

    if args.list_themes {
        for name in theme::Theme::list_builtins() {
            println!("{}", name);
        }
        return Ok(());
    }

    let input = args
        .input
        .expect("input is required unless --completions or --list-themes is used");
    let output = args.output.unwrap_or_else(|| {
        if input.to_str() == Some("-") {
            PathBuf::from("output.png")
        } else {
            input.with_extension("png")
        }
    });

    // Load theme: try built-in name first, then file path
    let theme = if let Some(ref theme_arg) = args.theme {
        if let Ok(builtin) = theme::Theme::from_builtin(theme_arg) {
            builtin
        } else {
            let theme_path = Path::new(theme_arg);
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
                return Err(format!(
                    "Unknown theme '{}'. Use --list-themes to see built-in themes, or provide a valid file path.",
                    theme_arg
                ));
            }
        }
    } else {
        theme::Theme::default()
    };

    // Read markdown input
    let markdown = if input.to_str() == Some("-") {
        let mut buffer = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
            .map_err(|e| format!("Failed to read from stdin: {}", e))?;
        buffer
    } else {
        std::fs::read_to_string(&input)
            .map_err(|e| format!("Failed to read input file: {}", e))?
    };

    let base_path = if input.to_str() == Some("-") {
        None
    } else {
        input.parent().map(|path| path.to_path_buf())
    };

    // Render to SVG
    let measure = fonts::CosmicTextMeasure::new()?;
    let mut renderer =
        renderer::Renderer::new_with_base_path(theme, measure, args.width, base_path)?;
    let svg = renderer.render(&markdown)?;

    // Save output in the requested format
    markie::export::save_output(&svg, &output, args.png_scale)?;

    Ok(())
}

# Markie

A pure Rust Markdown to SVG/PNG renderer that converts Markdown documents into beautiful, shareable images.

## Features

- **Pure Rust**: Built entirely with Rust for performance and reliability
- **Multiple Output Formats**: Export to SVG or PNG
- **Customizable Themes**: Support for custom themes via base64-encoded JSON
- **Flexible Input**: Read from file or stdin
- **Adjustable Width**: Control output image width
- **Font Support**: Includes local font directory and system font fallbacks

## Installation

### From source

```bash
cargo install --path .
```

### From crates.io (when published)

```bash
cargo install markie
```

## Usage

### Basic usage

Render a Markdown file to SVG:

```bash
markie input.md -o output.svg
```

Render to PNG:

```bash
markie input.md -o output.png
```

### From stdin

```bash
cat README.md | markie - -o output.svg
```

### Custom width

```bash
markie input.md -o output.png --width 1200
```

### With custom theme

```bash
markie input.md -o output.svg --theme "eyJmb250X2ZhbWlseSI6ICJBcmlhbC..."
```

### With Alacritty theme (YAML or TOML)

You can use any [Alacritty theme](https://github.com/alacritty/alacritty-theme) directly (both `.yaml` and `.toml` formats are supported):

```bash
markie input.md -o output.svg --theme alacritty.toml
```

## Theme Format

Themes can be passed as:
1. Base64-encoded JSON string
2. Path to an Alacritty theme file (YAML or TOML)

```json
{
  "background_color": "#ffffff",
  "text_color": "#333333",
  "font_family": "Arial",
  "font_size": 16,
  "line_height": 1.6,
  "heading_colors": {
    "h1": "#000000",
    "h2": "#333333",
    "h3": "#666666"
  }
}
```

## Building

```bash
cargo build --release
```

The binary will be available at `target/release/markie`.

## Dependencies

- `cosmic-text`: Text shaping and layout
- `pulldown-cmark`: Markdown parsing
- `resvg`: SVG rendering
- `tiny-skia`: Software rendering
- `clap`: Command-line argument parsing

## License

This project is provided as-is for educational and personal use.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

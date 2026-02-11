# Markie

![Example Output](README.svg)

A pure Rust Markdown to SVG/PNG renderer that converts Markdown documents into beautiful, shareable images.

## Features

- **Pure Rust**: Built entirely with Rust for performance and reliability
- **Multiple Output Formats**: Export to SVG or PNG
- **Customizable Themes**: Support for custom themes via Alacritty configuration files (YAML/TOML)
- **Flexible Input**: Read from file or stdin
- **Adjustable Width**: Control output image width
- **Font Support**: Includes local font directory and system font fallbacks

## Markdown Support

Supported today:

- Headings, paragraphs, emphasis/strong, inline code
- Fenced code blocks with syntax highlighting
- Lists (ordered/unordered) and task lists
- Blockquotes and horizontal rules
- Links (colored text)
- Strikethrough
- Tables
- Images (local files, data URLs, and remote HTTP/S sources)
- Inline and display math (rendered as monospace text)
- Footnotes
- Definition lists
- Inline HTML and HTML blocks (rendered as code)
- **Improved typography and spacing** for better visual consistency

### Feature Examples

#### Text Formatting

You can use **bold text**, *italic text*, and `inline code` for emphasis.

~~Strikethrough text~~ is also supported.

#### Links and Images

[Link text](https://example.com) renders as colored text.

Images from local files or URLs:
`![Alt text](image.png)`

#### Lists

Unordered lists:
- Item 1
- Item 2
  - Nested item
- Item 3

Ordered lists:
1. First item
2. Second item
3. Third item

Task lists:
- [x] Completed task
- [ ] Pending task

#### Code Blocks

```rust
fn main() {
    println!("Hello, Markie!");
}
```

#### Tables

| Column 1 | Column 2 | Column 3 |
|----------|----------|----------|
| Data 1   | Data 2   | Data 3   |
| Data 4   | Data 5   | Data 6   |

#### Blockquotes

> This is a blockquote.
> It can span multiple lines.

#### Math

Inline math: $E = mc^2$

Display math:
$$
\sum_{i=1}^{n} x_i
$$

#### Footnotes

This is a reference [^1] to a footnote.

[^1]: This is the footnote content.

#### Definition Lists

Term 1
: Definition 1

Term 2
: Definition 2

#### HTML

Inline `<span>` tags are rendered as code.

```html
<div>HTML blocks render as code blocks</div>
```

Not yet supported:

- Metadata blocks are parsed but ignored
- Mermaid or other diagram rendering (code blocks are rendered as code)
- Rich HTML rendering (HTML is rendered as inline code or code blocks)

## Installation

### From source

```bash
cargo install --path .
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

### With Alacritty theme (YAML or TOML)

You can use any Alacritty theme directly (both `.yaml` and `.toml` formats are supported).
A great collection of themes can be found at [alacritty-theme](https://github.com/alacritty/alacritty-theme).

```bash
markie input.md -o output.svg --theme solarized_light.toml
```

## Theme Format

Themes can be passed as a path to an Alacritty theme file (YAML or TOML).

Example of Alacritty TOML theme:

```toml
[colors.primary]
background = '#fdf6e3'
foreground = '#586e75'

[colors.normal]
black   = '#073642'
red     = '#dc322f'
green   = '#859900'
# ... other colors
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
- `syntect`: Syntax highlighting
- `clap`: Command-line argument parsing
- `serde`: Serialization/Deserialization (JSON, YAML, TOML)

## License

This project is provided as-is for educational and personal use.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

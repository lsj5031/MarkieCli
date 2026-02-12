# PROJECT KNOWLEDGE BASE

**Generated:** 2026-02-12
**Commit:** 88e5dde
**Branch:** main

## OVERVIEW

Pure Rust CLI tool rendering Markdown to SVG/PNG/PDF with native Mermaid diagrams and LaTeX math.

## STRUCTURE

```
markie/
├── src/
│   ├── main.rs          # CLI entry, arg parsing, format routing
│   ├── renderer.rs      # Core SVG rendering (~2000 lines)
│   ├── theme.rs         # Theme struct + Alacritty parsing
│   ├── math.rs          # LaTeX→MathML→SVG via latex2mathml
│   ├── fonts.rs         # Global font system with LRU cache
│   └── mermaid/         # Native diagram rendering subsystem
├── tests/fixtures/      # Theme files, test markdown
├── scripts/smoke-test.sh # Visual regression (all formats)
└── fonts/               # Local font directory
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Add output format | `main.rs:93-116` | Match on extension |
| Modify SVG rendering | `renderer.rs` | `Renderer` struct handles all elements |
| Add Mermaid diagram type | `src/mermaid/` | parser.rs → layout.rs → render.rs |
| Change math rendering | `math.rs` | `render_math()` entry point |
| Theme customization | `theme.rs` | `Theme` struct, `from_alacritty_*` |
| Font loading | `fonts.rs` | `GLOBAL_FONT_SYSTEM` singleton |
| Add CLI flag | `main.rs:16-36` | `Args` struct with clap |

## CODE MAP

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `Renderer` | struct | `renderer.rs:168` | Main rendering state machine |
| `Theme` | struct | `theme.rs:22` | Colors, fonts, spacing config |
| `TextMeasure` | trait | `fonts.rs:18` | Text measurement abstraction |
| `render_math` | fn | `math.rs:64` | LaTeX → SVG entry |
| `parse_mermaid` | fn | `mermaid/parser.rs:24` | Mermaid source → AST |
| `render_diagram` | fn | `mermaid/render.rs:53` | AST → SVG |
| `DiagramStyle` | struct | `mermaid/render.rs:7` | Theme-aware diagram colors |

## CONVENTIONS

- **Edition 2024** - Latest Rust edition
- **No lib.rs** - Binary-only crate; modules declared in `main.rs`
- **Old mod.rs pattern** - `mermaid/mod.rs` (not `mermaid.rs`)
- **Inline tests** - `#[cfg(test)] mod tests` in source files
- **Mock traits for testing** - `MockMeasure` in `renderer.rs:20-32`
- **Result<T, String>** - All errors as String (no error crate)
- **No dead code policy** - avoid `#[allow(dead_code)]`; keep enums/functions fully wired or remove unused paths

## COMMANDS

```bash
# Build
cargo build --release

# Run tests
cargo test

# Visual smoke test (generates SVG/PNG/PDF)
./scripts/smoke-test.sh ./smoke-output

# Render markdown
./target/release/markie input.md -o output.svg
./target/release/markie input.md -o output.png --png-scale 2
./target/release/markie input.md -o output.pdf --theme theme.toml
```

## NOTES

- **Font cache**: 100k entry LRU in `GLOBAL_FONT_SYSTEM` - expensive initial creation, then shared
- **Syntax highlighting**: Uses syntect with Solarized (dark/light) auto-selected based on code background
- **Mermaid native**: No JS runtime - pure Rust parser + layout + SVG render
- **Mermaid sequence parser**: Supports notes and nested control blocks (`alt/opt/loop/par/critical` with `else`/`end`)
- **Mermaid flowchart edges**: Supports circle/cross/open-arrow variants and bidirectional/thick/dotted operators
- **Mermaid state parser**: Tracks composite-state children (`State`, `Transition`, `Note`) to keep state AST paths live
- **PDF export**: Text as paths (`embed_text = false`) for viewer compatibility
- **HTML rendered as code**: `<div>` shows as inline code or code block, not rendered

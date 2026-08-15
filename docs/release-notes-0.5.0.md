# Markie v0.5.0

**Pure Rust Markdown → SVG/PNG/PDF renderer** with native Mermaid diagrams and LaTeX math.

35 commits since v0.4.0 across 32 files (+3,154 / −536 lines).

## Highlights

- **Inline HTML with real styling** — `<sup>`/`<sub>`, `<span style="color: …">`, `<font color>`, `<mark>`, and `<u>` now render with actual formatting instead of being dropped. Superscripts and subscripts get scaled, raised/lowered baselines; colors accept hex, `rgb()/rgba()/hsl()`, and CSS named values.
- **Theme-aware `<mark>` highlight** — classic yellow on light themes, dark olive-amber on dark themes (pure yellow left light text at ~1:1 contrast — unreadable). Text on the highlight is now ≥3:1 contrast on every demo theme.
- **No more lost content** — HTML blocks render as syntax-highlighted code blocks instead of silently vanishing; long table cells word-wrap (with hard-splitting) so tables can never overflow the page.
- **Hardened image fetching** — remote images get a 10-second timeout and a 10 MiB size limit; two security fixes from earlier in the cycle are also included (absolute image-path sanitization, MathML panic fix).
- **Strict Mermaid diagram types** — `pie`/`gantt`/etc. now fail with a clear "unsupported diagram type" error listing what *is* supported, instead of silently producing an empty flowchart.
- **Faster rendering** — source-line tracking is now O(log n) per parse event instead of O(n²), and space-width measurement is memoized per style.

## What's changed

See [CHANGELOG.md](../CHANGELOG.md) for the full categorized list (Features, Security, Bug Fixes, Performance, Refactoring, Documentation, Testing).

## Demos

All demo assets were regenerated from [`demo-all-features.md`](../demo-all-features.md) and are committed in every format — SVG, 2× PNG, and PDF, across the default Solarized Light plus Dracula, Nord, Catppuccin Mocha, and Solarized Dark themes. Browse them interactively in the new [demo gallery](../docs/gallery.html), or flip through the screenshots in the [README](../README.md#screenshots).

## Install

```bash
cargo install markie          # or: cargo binstall markie
```

Prebuilt binaries for Linux (x86_64, aarch64), macOS (x86_64, Apple Silicon), and Windows are attached to this release.

```bash
markie input.md -o output.png # default: PNG at 2× retina
markie input.md -o output.svg
markie input.md -o output.pdf --theme dracula
```

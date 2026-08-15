# Changelog

All notable changes to MarkieCli will be documented in this file.

## [Unreleased]

### Features

- **Basic inline HTML styling** — inline `<span style="color/background/font-size/text-decoration">`, `<font color>`, `<sup>`, `<sub>`, `<u>`, and `<mark>` tags now style their content instead of being dropped. Colors accept hex, `rgb()/rgba()/hsl()`, and CSS named values; values are sanitized so they can never inject markup into the SVG.
- **Theme-aware `<mark>` highlight** — the default mark highlight is classic yellow on light themes and a dark olive-amber on dark themes, where pure yellow left light text at ~1:1 contrast. Verified contrast is ≥3:1 against text on all demo themes (Dracula 9.1:1, Nord 7.2:1, Catppuccin 6.7:1, Solarized Dark 3.1:1).
- **Continuous highlights and underlines across spaces** — `<mark>a b</mark>` and `<u>a b</u>` no longer leave a gap where the space is; background rects and underline segments now span whitespace inside their scope (also applies to link underlines).

### Bug Fixes

- **Render HTML blocks as code instead of dropping them** — HTML blocks were silently discarded during rendering, contradicting the documented behavior. They now render as syntax-highlighted code blocks so no content is lost.
- **Tables never overflow the output width** — long cell content now word-wraps (with hard-splitting of unbreakable words), and oversized tables shrink proportionally to fit the page instead of extending past the right edge. Row heights grow per-row to fit wrapped text.
- **Bound remote image fetches** — remote images now use a 10-second timeout and a 10 MiB size limit, preventing hung renders and memory exhaustion from slow or oversized image servers.

### Performance

- **Fix O(n²) source-line tracking** — the renderer no longer re-counts newlines from the start of the document for every parse event; line numbers are resolved via a precomputed line index in O(log n) per event.
- **Memoize space-width inference** — the three-measurement `"m m"`/`"mm"` space-advance inference now runs once per (font size, weight, style) instead of once per space character; subsequent spaces are a single hash lookup.

### Bug Fixes

- **Unknown Mermaid diagram types now error clearly** — `pie`, `gantt`, and other unsupported diagram types previously fell back to a flowchart parse (with only a warning), producing a confusing empty diagram. They now fail with `Unsupported Mermaid diagram type '...'` listing the supported types. Type detection also skips leading `%%` comments/init configs, and empty diagram fences still render without error.

### Documentation

- Add an inline-HTML showcase section to `demo-all-features.md` and regenerate all demo assets (`demo-all-features.svg/png/pdf`, plus the Dracula, Nord, Catppuccin, and Solarized Dark SVGs), which also fixes the previously stale checked-in demos.
- Point the README screenshots at the regenerated demo files — the hero now uses the PNG (inline SVGs render blank in several markdown viewers) with links to the SVG/PDF, and the themed-variant gallery moved into a dedicated Screenshots section that links every committed format.
- Commit 2× PNG rasterizations of the four dark-themed demos (`demo-dracula.png`, `demo-nord.png`, `demo-catppuccin.png`, `demo-solarized-dark.png`) alongside their SVGs so the README gallery renders in viewers without SVG support; `make-demo.sh` now emits both formats for each theme.
- Add [`docs/gallery.html`](docs/gallery.html) — a dependency-free, lightbox-style gallery embedding every committed demo asset (all themes, SVG/PNG/PDF) with format filters, keyboard navigation, and per-item links to the original files.
- Add `scripts/make-demo.sh` to regenerate every committed demo from `demo-all-features.md`, so the demos can't go stale again.

### Refactoring

- Deduplicate the duplicated font-fallback selection logic in `export.rs` into a shared helper.
- Remove a dead text-measurement call in inline-code rendering and an unnecessary `#[allow(dead_code)]` attribute.

## [0.5.0] - 2026-04-04

### Security

- **Fix arbitrary local file read via absolute image paths** — prevent malicious markdown documents from reading arbitrary files on the host by sanitizing absolute paths in image references (`722b7a6`)
- **Fix potential panic in MathML parser** — replace fragile `unwrap()` with `unwrap_or_default()` to prevent crashes on malformed MathML input (`7960bfe`)

### Performance

- Optimize string allocation in character measurement loop (`fd7c6ed`)
- Optimize ER diagram entity auto-creation (`6f637e3`)

### Refactoring

- Refactor `render_token` to reduce cyclomatic complexity and improve maintainability (`f4286e0`)
- Refactor output format handling into a shared utility function (`095ca58`)
- Remove unused `cache_len` and `cache_size` functions (`abdcc78`)

### Bug Fixes

- Address code review issues across renderer, math, mermaid, and theme modules (`b8a4ee1`)
- Fix compilation error by adding `#[derive(Debug)]` to `MathResult` (`52bfc54`)
- Fix unused import warnings (`deaed2c`)

### Documentation

- Add comprehensive demo files for all themes and features (`f4f039d`, `aff4c09`)
- Add `demo-all-features.md` showcase with generated SVG/PNG/PDF outputs
- Add theme-specific demo SVGs: Catppuccin, Dracula, Nord, Solarized Dark
- Update README with refreshed screenshots
- Remove legacy `examples.md` in favor of unified demo

### Testing

- Add error path tests for `save_output` in export module (`73dda3e`)
- Add error path tests for Mermaid parser (`52526f6`, `ea73ed9`, `cdf1c4f`)
- Add unit tests for `render_math` API (`54486b9`)

### Summary

28 commits since v0.4.0 across 23 files (+1,374 / −406 lines). This release focuses on security hardening (two vulnerability fixes), performance optimizations, code quality improvements through refactoring, and expanded test coverage.

---

## [0.4.0] - 2026-03-10

### Features

- Complete Phase 4 layout engine with enhanced `EdgeLabelPlacer`
- Add Phase 2 property-based tests and Phase 3 `GlyphBox` layout module
- Add `cargo binstall` metadata and one-line installer

### Security

- Prevent path traversal in image path resolution

### Bug Fixes

- Resolve Mermaid diagram and markdown rendering issues
- Apply Phase 1 quick wins for text overlap prevention
- Fix definition list spacing consistency
- Avoid panic in `Theme::default` via safe fallback

### Refactoring

- Extract SVG export logic into shared module
- Fix clippy warnings and refactor rendering contexts

---

## [0.3.0] - 2026-03-04

### Features

- Add theme support with multiple built-in themes
- Add Mermaid diagram support (flowchart, sequence, class, ER diagrams)
- Add math rendering (LaTeX-style inline and display math)

---

## [0.2.0] - 2026-02-20

### Features

- Add PDF export support
- Add SVG export with embedded fonts
- Improve rendering pipeline with proper text measurement

---

## [0.1.9] - 2026-02-10

### Features

- Initial public release
- Markdown to image conversion (PNG/SVG)
- Syntax highlighting for code blocks
- Custom font support

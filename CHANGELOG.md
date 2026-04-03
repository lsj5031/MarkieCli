# Changelog

All notable changes to MarkieCli will be documented in this file.

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

# TDD Refactoring Plan for MarkieCli Layout System

**Created:** 2026-03-09
**Updated:** 2026-03-09
**Status:** Phase 1 Complete

## Problem Statement

The current layout implementation has text overlap issues caused by:
1. Token-by-token rendering without tracking visual bounding boxes
2. Simple line height multipliers that don't account for actual font metrics
3. Fixed offsets for inline code boxes that may not match font metrics
4. Mermaid edge label collision detection at placement time only

## Phase 1: Quick Wins (Low Risk) ✅ COMPLETE

Commit: `65b69d0`

### 1.1 Line Height Safety Margins ✅

**File:** `src/renderer.rs`
**Location:** `current_line_height()` method

**Implemented:**
```rust
fn current_line_height(&self) -> f32 {
    if self.heading_level.is_some() {
        self.theme.line_height.max(1.35)
    } else {
        self.theme.line_height.max(1.4)
    }
}
```

**Tests added:** `test_line_height_has_safety_margin_for_body_text`

### 1.2 Descent Padding in advance_line ✅

**File:** `src/renderer.rs`
**Location:** `advance_line()` method

**Implemented:**
```rust
fn advance_line(&mut self, font_size: f32) {
    let descent_padding = font_size * 0.15;
    self.cursor_y += font_size * self.current_line_height() + descent_padding;
    self.cursor_x = self.line_start_x();
    self.at_line_start = true;
}
```

**Tests added:** `test_line_advance_includes_descent_padding`

### 1.3 Inline Code Box Alignment Fix ✅

**File:** `src/renderer.rs`
**Location:** `render_inline_code()` method

**Implemented:**
```rust
let ascent_ratio = 0.75;
let rect_y = self.cursor_y - self.theme.font_size_code * ascent_ratio - self.theme.code_padding_y * 0.5;
```

**Tests added:** `test_inline_code_box_alignment_uses_ascent_ratio`, `test_consecutive_inline_code_no_overlap`

**New Code:**
```rust
fn advance_line(&mut self, font_size: f32) {
    let descent_padding = font_size * 0.15;
    self.cursor_y += font_size * self.current_line_height() + descent_padding;
    self.cursor_x = self.line_start_x();
    self.at_line_start = true;
}
```

**Tests to add:**
- Test that advance_line adds descent padding
- Test descent padding scales with font size

### 1.3 Inline Code Box Alignment Fix

**File:** `src/renderer.rs`
**Location:** `render_inline_code()` method (~line 683)

**Current Code:**
```rust
let rect_y = self.cursor_y - self.theme.font_size_code * 0.8 - self.theme.code_padding_y / 2.0;
```

**New Code:**
```rust
// Use font metrics for proper alignment
let ascent_ratio = 0.75; // Typical ascent ratio for most fonts
let rect_y = self.cursor_y - self.theme.font_size_code * ascent_ratio - self.theme.code_padding_y * 0.5;
```

## Phase 2: Property-Based Tests (Medium Risk)

### 2.1 Add proptest Dependency

**File:** `Cargo.toml`

```toml
[dev-dependencies]
proptest = "1.5"
```

### 2.2 Layout Invariant Tests

**File:** `src/renderer.rs` (in tests module)

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_line_spacing_prevents_overlap(
            font_size in 10.0f32..32.0,
            line_height in 1.0f32..2.5,
        ) {
            let theme = Theme {
                line_height,
                ..Theme::default()
            };
            let measure = MockMeasure;
            let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

            let line1_y = renderer.cursor_y;
            renderer.advance_line(font_size);
            let line2_y = renderer.cursor_y;

            // Next line should start with enough gap to prevent visual overlap
            // Minimum gap should be at least font_size * 0.1 (descent margin)
            prop_assert!(line2_y > line1_y + font_size * 0.9);
        }

        #[test]
        fn test_text_elements_dont_overlap_horizontally(
            words in prop::collection::vec("[a-zA-Z]{1,10}", 2..5),
        ) {
            let theme = Theme::default();
            let measure = MockMeasure;
            let mut renderer = Renderer::new(theme, measure, 800.0).unwrap();

            let markdown = words.join(" ");
            let result = renderer.render(&markdown);
            prop_assert!(result.is_ok());

            let svg = result.unwrap();
            // Extract text element x positions and verify ordering
            // (Implementation depends on SVG parsing)
        }
    }
}
```

## Phase 3: GlyphBox-Based Layout (Higher Risk)

### 3.1 Define Layout Contracts

**New File:** `src/layout.rs`

```rust
use crate::fonts::TextMeasure;

/// Represents the actual visual bounds of rendered text
#[derive(Debug, Clone, Copy)]
pub struct GlyphBox {
    /// X position of the glyph origin
    pub x: f32,
    /// Y position of the baseline
    pub y: f32,
    /// Advance width (how much cursor moves)
    pub advance_width: f32,
    /// Actual ink bounds (for collision detection)
    pub ink_bounds: Rect,
    /// Font ascent (distance from baseline to top)
    pub ascent: f32,
    /// Font descent (distance from baseline to bottom)
    pub descent: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Trait for text layout engines
pub trait TextLayout {
    /// Measure text and return detailed glyph information
    fn measure_glyphs(
        &mut self,
        text: &str,
        font_size: f32,
        is_code: bool,
        is_bold: bool,
        is_italic: bool,
    ) -> Vec<GlyphBox>;

    /// Layout a line of text tokens
    fn layout_line(
        &mut self,
        tokens: &[&str],
        max_width: f32,
        start_x: f32,
        baseline_y: f32,
        font_size: f32,
    ) -> Vec<GlyphBox>;
}

/// Check if two glyph boxes overlap visually
pub fn boxes_overlap(a: &GlyphBox, b: &GlyphBox) -> bool {
    a.ink_bounds.x < b.ink_bounds.x + b.ink_bounds.width
        && a.ink_bounds.x + a.ink_bounds.width > b.ink_bounds.x
        && a.ink_bounds.y < b.ink_bounds.y + b.ink_bounds.height
        && a.ink_bounds.y + a.ink_bounds.height > b.ink_bounds.y
}
```

### 3.2 Implement TextLayoutEngine

**File:** `src/layout.rs`

```rust
use crate::fonts::TextMeasure;

pub struct TextLayoutEngine<T: TextMeasure> {
    measure: T,
}

impl<T: TextMeasure> TextLayoutEngine<T> {
    pub fn new(measure: T) -> Self {
        Self { measure }
    }
}

impl<T: TextMeasure> TextLayout for TextLayoutEngine<T> {
    fn measure_glyphs(
        &mut self,
        text: &str,
        font_size: f32,
        is_code: bool,
        is_bold: bool,
        is_italic: bool,
    ) -> Vec<GlyphBox> {
        let (width, _height) = self.measure.measure_text(
            text, font_size, is_code, is_bold, is_italic, None
        );

        // Estimate ascent/descent based on font metrics
        // In production, this should use actual font metrics from cosmic-text
        let ascent = font_size * 0.8;
        let descent = font_size * 0.2;

        vec![GlyphBox {
            x: 0.0,
            y: 0.0,
            advance_width: width,
            ink_bounds: Rect {
                x: 0.0,
                y: -ascent,
                width,
                height: ascent + descent,
            },
            ascent,
            descent,
        }]
    }

    fn layout_line(
        &mut self,
        tokens: &[&str],
        max_width: f32,
        start_x: f32,
        baseline_y: f32,
        font_size: f32,
    ) -> Vec<GlyphBox> {
        let mut boxes = Vec::new();
        let mut current_x = start_x;

        for token in tokens {
            let glyph = self.measure_glyphs(token, font_size, false, false, false);
            if let Some(mut g) = glyph.into_iter().next() {
                g.x = current_x;
                g.y = baseline_y;
                g.ink_bounds.x = current_x;
                g.ink_bounds.y = baseline_y - g.ascent;

                boxes.push(g);
                current_x += g.advance_width;
            }
        }

        boxes
    }
}
```

### 3.3 Add Layout Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct TestMeasure;
    impl TextMeasure for TestMeasure {
        fn measure_text(
            &mut self,
            text: &str,
            font_size: f32,
            _is_code: bool,
            _is_bold: bool,
            _is_italic: bool,
            _max_width: Option<f32>,
        ) -> (f32, f32) {
            (text.len() as f32 * font_size * 0.6, font_size)
        }
    }

    #[test]
    fn test_glyph_boxes_have_correct_dimensions() {
        let mut engine = TextLayoutEngine::new(TestMeasure);
        let glyphs = engine.measure_glyphs("Hello", 16.0, false, false, false);

        assert_eq!(glyphs.len(), 1);
        assert!(glyphs[0].advance_width > 0.0);
        assert!(glyphs[0].ascent > 0.0);
        assert!(glyphs[0].descent > 0.0);
    }

    #[test]
    fn test_adjacent_glyphs_dont_overlap() {
        let mut engine = TextLayoutEngine::new(TestMeasure);
        let boxes = engine.layout_line(
            &["Hello", "World"],
            800.0,
            0.0,
            16.0,
            16.0,
        );

        assert!(boxes.len() >= 2);
        assert!(!boxes_overlap(&boxes[0], &boxes[1]));
    }

    #[test]
    fn test_boxes_overlap_detection() {
        let a = GlyphBox {
            x: 0.0, y: 0.0, advance_width: 50.0,
            ink_bounds: Rect { x: 0.0, y: -12.0, width: 50.0, height: 16.0 },
            ascent: 12.0, descent: 4.0,
        };

        let b_overlapping = GlyphBox {
            x: 40.0, y: 0.0, advance_width: 50.0,
            ink_bounds: Rect { x: 40.0, y: -12.0, width: 50.0, height: 16.0 },
            ascent: 12.0, descent: 4.0,
        };

        let b_not_overlapping = GlyphBox {
            x: 60.0, y: 0.0, advance_width: 50.0,
            ink_bounds: Rect { x: 60.0, y: -12.0, width: 50.0, height: 16.0 },
            ascent: 12.0, descent: 4.0,
        };

        assert!(boxes_overlap(&a, &b_overlapping));
        assert!(!boxes_overlap(&a, &b_not_overlapping));
    }
}
```

## Phase 4: Mermaid Label Collision Fix

### 4.1 Add Label Router

**File:** `src/mermaid/layout.rs`

```rust
/// Edge label placement with collision avoidance
pub struct EdgeLabelPlacer {
    occupied_regions: Vec<BBox>,
    padding: f32,
}

impl EdgeLabelPlacer {
    pub fn new(padding: f32) -> Self {
        Self {
            occupied_regions: Vec::new(),
            padding,
        }
    }

    pub fn reserve(&mut self, bbox: BBox) {
        self.occupied_regions.push(bbox.with_padding(self.padding));
    }

    pub fn find_position(&self, preferred: (f32, f32), label_size: (f32, f32)) -> (f32, f32) {
        let (mut x, mut y) = preferred;
        let (w, h) = label_size;

        // Try the preferred position first
        if !self.collides(x, y, w, h) {
            return (x, y);
        }

        // Try offsetting vertically
        for offset in [10.0, 20.0, 30.0, -10.0, -20.0, -30.0] {
            if !self.collides(x, y + offset, w, h) {
                return (x, y + offset);
            }
        }

        // Try offsetting horizontally
        for offset in [10.0, 20.0, 30.0, -10.0, -20.0, -30.0] {
            if !self.collides(x + offset, y, w, h) {
                return (x + offset, y);
            }
        }

        // Fallback to preferred position
        (x, y)
    }

    fn collides(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let proposed = BBox::new(x, y, w, h);
        self.occupied_regions.iter().any(|r| {
            proposed.x < r.x + r.width
                && proposed.x + proposed.width > r.x
                && proposed.y < r.y + r.height
                && proposed.y + proposed.height > r.y
        })
    }
}
```

## Implementation Order

1. ✅ Phase 1.1: Line height safety margins
2. ✅ Phase 1.2: Descent padding
3. ✅ Phase 1.3: Inline code box fix
4. Phase 2: Add property-based tests
5. Phase 3: Extract layout contracts (optional, larger refactoring)
6. Phase 4: Mermaid label collision fix (optional)

## Verification

After each phase:
1. Run `cargo test`
2. Run visual smoke test: `./scripts/smoke-test.sh ./test-output`
3. Check for overlapping text in generated SVGs

## Estimated Effort

- Phase 1 (Quick Wins): 1-2 hours
- Phase 2 (Property Tests): 2-3 hours
- Phase 3 (Layout Refactor): 4-6 hours
- Phase 4 (Mermaid Labels): 2-3 hours

Total: 9-14 hours for complete refactoring

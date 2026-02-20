# The Theme System

> This document explains how Markie handles visual styling through its theme system.

## What is a Theme?

A theme is a collection of visual properties that control how your Markdown document looks when rendered. Think of it as a "color scheme" or "style preset" that affects colors, fonts, and spacing.

```mermaid
flowchart TB
    A[Theme File] --> B[Markie]
    B --> C[Styled Output]
    
    subgraph Before["Without Theme"]
        D1[Default Colors]
        D2[Default Fonts]
    end
    
    subgraph After["With Theme"]
        T1[Custom Colors]
        T2[Custom Fonts]
        T3[Custom Spacing]
    end
    
    A --> After
```

## Theme Properties

The theme controls every visual aspect of the output:

```mermaid
flowchart TB
    ROOT((Theme Properties))
    ROOT --> COLORS[Colors]
    ROOT --> TYPO[Typography]
    ROOT --> SPACING[Spacing]
    ROOT --> CODE[Code Blocks]
```

## Default Theme: GitHub Light

Markie ships with a default theme inspired by GitHub's light mode:

```mermaid
flowchart TB
    subgraph GitHubLight["GitHub Light Theme Colors"]
        BG["Background: #ffffff"] --> TEXT["Text: #24292f"]
        TEXT --> HEADING["Heading: #1b1f23"]
        HEADING --> LINK["Link: #0969da"]
        LINK --> CODE_BG["Code BG: #f6f8fa"]
    end
```

## Theme File Formats

Markie supports Alacritty theme files in both YAML and TOML formats:

```mermaid
flowchart TB
    FILE[Theme File] --> EXT{File Extension?}
    
    EXT -->|.yaml| YAML[Parse as YAML]
    EXT -->|.toml| TOML[Parse as TOML]
    EXT -->|other| ERROR[Unsupported Format]
    
    YAML --> ALACRITTY[AlacrittyTheme Struct]
    TOML --> ALACRITTY
    
    ALACRITTY --> CONVERT[Convert to Theme]
    CONVERT --> THEME[Theme Struct]
```

### TOML Format Example

```mermaid
flowchart TB
    subgraph TOMLFile["theme.toml"]
        CONTENT["TOML theme configuration"]
    end
    
    TOMLFile --> MAPPING["Color Mapping to Theme"]
```

### YAML Format Example

```mermaid
flowchart TB
    subgraph YAMLFile["theme.yaml"]
        CONTENT["YAML theme configuration"]
    end
    
    YAMLFile --> MAPPING["Same Mapping as TOML"]
```

## Color Mapping Logic

Alacritty themes don't have all the colors Markie needs, so we map them intelligently:

```mermaid
flowchart TB
    ALAC[Alacritty Colors] --> MAP[Color Mapping]
    
    MAP --> M1["background ← primary.background"]
    MAP --> M2["text ← primary.foreground"]
    MAP --> M3["heading ← normal.blue"]
    MAP --> M4["link, code, quote ← other fields"]
    
    M1 --> THEME[Theme Struct]
    M2 --> THEME
    M3 --> THEME
    M4 --> THEME
```

## Theme Loading Process

```mermaid
sequenceDiagram
    participant CLI as Command Line
    participant Main as main.rs
    participant Theme as theme.rs
    participant FS as File System
    
    CLI->>Main: --theme path/to/theme.toml
    
    Main->>FS: Check if file exists
    FS-->>Main: File exists
    
    Main->>FS: Read file contents
    FS-->>Main: TOML string
    
    Main->>Theme: from_alacritty_toml(contents)
    Theme->>Theme: Parse TOML to AlacrittyTheme
    Theme->>Theme: Convert to Theme struct
    Theme-->>Main: Theme instance
    
    Note over Main: Theme loaded successfully
    
    alt Parse Failed
        Theme-->>Main: Error
        Main->>Main: Return error to user
    end
```

## Theme Application

Once loaded, the theme is used throughout the rendering process:

```mermaid
flowchart TB
    THEME[Theme] --> RENDERER[Renderer]
    
    RENDERER --> BG["Background → background_color"]
    RENDERER --> TEXT["Text → text_color"]
    RENDERER --> HEADINGS["Headings → heading_color"]
    RENDERER --> CODE["Code → code_bg + code_text"]
    RENDERER --> QUOTES["Quotes → quote_border_color"]
```

### Color Usage by Element

```mermaid
flowchart TB
    ROOT[Root SVG] --> HEADER["Heading → heading_color"]
    ROOT --> PARA["Paragraph → text_color"]
    ROOT --> CODE_B["Code Block → code_bg + code_text"]
    ROOT --> QUOTE["Blockquote → quote_border + quote_text"]
    ROOT --> LINK_E["Link → link_color"]
```

## Typography Settings

```mermaid
flowchart TB
    BASE["font_size_base: 16px"] --> H1["H1: 32px"]
    BASE --> H2["H2: 24px"]
    BASE --> H3["H3: 20px"]
    BASE --> CODE["font_size_code: 13px"]
    BASE --> LH["line_height: 1.6"]
```

## Spacing System

```mermaid
flowchart TB
    MARGINS["Margins: 16px top/bottom"]
    PADDING["Padding: 32px x/y"]
    CODE_P["Code Padding: 12px × 8px"]
    RADIUS["Border Radius: 4px"]
    MARGINS --> PADDING --> CODE_P --> RADIUS
```

### Visual Spacing Guide

```mermaid
flowchart TB
    subgraph Page["Page Layout"]
        direction TB
        PY_TOP["padding_y (32px)"]
        
        subgraph Content["Content Area"]
            direction TB
            PX_LEFT["padding_x"]
            subgraph Text["Text Area"]
                M_TOP["margin_top"]
                ELEMENT["Element"]
                M_BOTTOM["margin_bottom"]
            end
            PX_RIGHT["padding_x"]
        end
        
        PY_BOTTOM["padding_y (32px)"]
    end
```

## Syntax Highlighting and Themes

Code block syntax highlighting automatically adapts to the theme's brightness:

```mermaid
flowchart TB
    CODE_BG[code_bg_color] --> LUM[Calculate Luminance]
    LUM --> BRIGHT{Is Dark?}
    
    BRIGHT -->|Yes, L < 128| DARK[Use Solarized Dark]
    BRIGHT -->|No, L >= 128| LIGHT[Use Solarized Light]
    
    DARK --> HIGHLIGHT[Apply Syntax Highlighting]
    LIGHT --> HIGHLIGHT
    
    subgraph LuminanceCalc["Luminance Calculation"]
        FORMULA["L = 0.299×R + 0.587×G + 0.114×B"]
        DARK_CHECK["Dark if L < 128"]
    end
    
    LUM --> LuminanceCalc
```

## Mermaid Diagram Styling

Mermaid diagram colors are derived from the theme:

```mermaid
flowchart TB
    THEME[Theme Colors] --> CONTRAST[pick_higher_contrast]
    CONTRAST --> FG[Select Best Foreground]
    FG --> MIX[mix_color at various ratios]
    MIX --> FILL["node_fill: 3%"]
    MIX --> STROKE["node_stroke: 20%"]
    MIX --> EDGE["edge_stroke: 30%"]
    MIX --> LABEL["edge_text: 60%"]
```

### Contrast Selection

```mermaid
flowchart TB
    BASE[code_bg_color] --> C1[Contrast with text_color]
    BASE --> C2[Contrast with background_color]
    
    C1 --> RATIO1[Contrast Ratio 1]
    C2 --> RATIO2[Contrast Ratio 2]
    
    RATIO1 --> COMP{Which is Higher?}
    RATIO2 --> COMP
    
    COMP -->|Ratio 1| USE_FG[Use text_color as diagram foreground]
    COMP -->|Ratio 2| USE_BG[Use background_color as diagram foreground]
```

## Theme Inheritance and Defaults

```mermaid
flowchart TB
    FILE[Theme File] --> PARSE[Parse File]
    PARSE --> SUCCESS{Success?}
    
    SUCCESS -->|Yes| USE_PARSED[Use Parsed Theme]
    SUCCESS -->|No| FALLBACK[Use Default]
    
    USE_PARSED --> MERGE[Merge with Defaults]
    MERGE --> FINAL[Final Theme]
    FALLBACK --> FINAL
    
    subgraph DefaultValues["Default Values for Missing Fields"]
        D_BG["background: #ffffff"]
        D_TEXT["text: #24292f"]
        D_FONT["font_size_base: 16.0"]
        D_MARGIN["margins: 16.0"]
    end
    
    MERGE --> DefaultValues
```

## Complete Theme Example

Here's a complete example showing how a theme affects the output:

```mermaid
flowchart TB
    INPUT["Sample Markdown Input"]
    INPUT --> LIGHT[Solarized Light Output]
    INPUT --> DARK[Dracula Dark Output]
    
    LIGHT --> SL["BG=#fdf6e3, Text=#586e75"]
    DARK --> DR["BG=#282a36, Text=#f8f8f2"]
```

## Using Themes from the Command Line

```mermaid
flowchart TB
    CMD[Command] --> PARSE[Parse Arguments]
    PARSE --> THEME_ARG{--theme flag?}
    
    THEME_ARG -->|Yes| LOAD[Load Theme File]
    THEME_ARG -->|No| DEFAULT[Use Default Theme]
    
    LOAD --> READ[Read File]
    READ --> DETECT{YAML or TOML?}
    DETECT -->|YAML| PARSE_YAML[Parse YAML]
    DETECT -->|TOML| PARSE_TOML[Parse TOML]
    
    PARSE_YAML --> CONVERT[Convert to Theme]
    PARSE_TOML --> CONVERT
    DEFAULT --> CONVERT
    
    CONVERT --> RENDER[Render with Theme]
```

### Command Examples

```mermaid
flowchart TB
    subgraph Commands["Theme Commands"]
        C1["markie input.md -o out.svg"]
        C2["markie input.md -o out.svg --theme light.toml"]
        C3["markie input.md -o out.png --theme dark.yaml"]
        C4["markie input.md -o out.pdf --theme solarized.toml"]
    end
```

---

*Previous: [Mermaid Subsystem](03-mermaid-subsystem.md)*
*Next: [Output Formats](05-output-formats.md)*

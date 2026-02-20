# Markie Architecture Overview

> This document explains the overall architecture of Markie - a pure Rust Markdown to SVG/PNG/PDF renderer.

## What is Markie?

Markie is a command-line tool that converts Markdown documents into beautiful images. Think of it as a "Markdown printer" - you give it a text file written in Markdown, and it produces a visually formatted image.

```mermaid
flowchart TB
    A[Markdown File] --> B[Markie]
    B --> C[SVG Image]
    B --> D[PNG Image]
    B --> E[PDF Document]
```

## Core Value Proposition

```mermaid
mindmap
  root((Markie))
    Zero Runtime
      Single binary
      No Node.js
      No Python
    Multiple Formats
      SVG vector
      PNG raster
      PDF document
    Rich Content
      Full Markdown
      Mermaid diagrams
      LaTeX math
      Syntax highlighting
    Easy Deployment
      Copy and run
      No npm install
      Cross-platform
```

## High-Level Architecture

Markie follows a **pipeline architecture** - data flows through a series of processing stages, each responsible for a specific transformation.

```mermaid
flowchart TB
    subgraph Input["📥 Input Stage"]
        MD[Markdown Text]
        THEME[Theme File]
    end
    
    subgraph Parsing["🔍 Parsing Stage"]
        PARSER[Markdown Parser]
        EVENTS[Event Stream]
    end
    
    subgraph Rendering["🎨 Rendering Stage"]
        RENDERER[SVG Renderer]
        FONTS[Font System]
        SYNTAX[Syntax Highlighter]
        MATH[Math Renderer]
        MERMAID[Mermaid Renderer]
    end
    
    subgraph Output["📤 Output Stage"]
        SVG[SVG String]
        PNG_CONVERT[PNG Converter]
        PDF_CONVERT[PDF Converter]
    end
    
    MD --> PARSER
    PARSER --> EVENTS
    EVENTS --> RENDERER
    THEME --> RENDERER
    RENDERER --> SVG
    SVG --> PNG_CONVERT
    SVG --> PDF_CONVERT
    FONTS --> RENDERER
    SYNTAX --> RENDERER
    MATH --> RENDERER
    MERMAID --> RENDERER
```

## Component Overview

### 1. Entry Point (`main.rs`)

The entry point handles command-line arguments and orchestrates the rendering pipeline.

```mermaid
flowchart TD
    START[Program Start] --> PARSE[Parse CLI Arguments]
    PARSE --> LOAD_THEME{Theme Specified?}
    LOAD_THEME -->|Yes| READ_THEME[Read Theme File]
    LOAD_THEME -->|No| DEFAULT_THEME[Use Default Theme]
    READ_THEME --> PARSE_THEME[Parse YAML/TOML]
    PARSE_THEME --> READ_MD
    DEFAULT_THEME --> READ_MD[Read Markdown Input]
    READ_MD --> CREATE_RENDERER[Create Renderer Instance]
    CREATE_RENDERER --> RENDER[Render to SVG]
    RENDER --> DETERMINE{Output Format?}
    DETERMINE -->|.svg| WRITE_SVG[Write SVG File]
    DETERMINE -->|.png| CONVERT_PNG[Convert to PNG]
    DETERMINE -->|.pdf| CONVERT_PDF[Convert to PDF]
    WRITE_SVG --> DONE[Complete]
    CONVERT_PNG --> DONE
    CONVERT_PDF --> DONE
```

### 2. Theme System (`theme.rs`)

The theme system controls all visual aspects of the output.

```mermaid
classDiagram
    class Theme {
        +String background_color
        +String text_color
        +String heading_color
        +String link_color
        +String code_bg_color
        +String code_text_color
        +String quote_border_color
        +String quote_text_color
        +float font_size_base
        +float font_size_code
        +float line_height
        +float margin_top
        +float margin_bottom
        +float padding_x
        +float padding_y
        +float code_padding_x
        +float code_padding_y
        +float code_radius
        +from_alacritty_yaml() Theme
        +from_alacritty_toml() Theme
        +github_light() Theme
    }
    
    class AlacrittyTheme {
        +AlacrittyColors colors
    }
    
    class AlacrittyColors {
        +AlacrittyPrimary primary
        +AlacrittyNormal normal
    }
    
    AlacrittyTheme --> Theme : converts to
    AlacrittyColors --> AlacrittyTheme : contains
```

### 3. Font System (`fonts.rs`)

The font system provides text measurement capabilities with intelligent caching.

```mermaid
flowchart TB
    subgraph GlobalSystem["Global Font System (Singleton)"]
        FS[FontSystem]
        CACHE[LRU Cache]
    end
    
    MEASURE[Text Measurement Request] --> CHECK{In Cache?}
    CHECK -->|Yes| RETURN[Return Cached Result]
    CHECK -->|No| COMPUTE[Compute Measurement]
    COMPUTE --> STORE[Store in Cache]
    STORE --> RETURN
    
    subgraph MeasureResult["Measurement Result"]
        WIDTH[Width in pixels]
        HEIGHT[Height in pixels]
    end
    
    RETURN --> MeasureResult
```

### 4. Core Renderer (`renderer.rs`)

The heart of Markie - processes Markdown events and generates SVG output.

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Processing : Start
    Processing --> InHeading : Heading
    Processing --> InCodeBlock : Code
    Processing --> InList : List
    Processing --> InTable : Table
    
    InHeading --> Processing : end
    InCodeBlock --> Processing : end
    InList --> Processing : end
    InTable --> Processing : end
    
    Processing --> Done : Complete
    Done --> [*]
```

### 5. Math Renderer (`math.rs`)

Converts LaTeX math expressions to SVG.

```mermaid
flowchart TB
    LATEX[LaTeX String] --> PARSE[MathML Parser]
    PARSE --> MATHML[MathML AST]
    MATHML --> LAYOUT[Layout Engine]
    LAYOUT --> SVG[SVG Fragment]
    
    subgraph Supported["Supported Features"]
        S1[Fractions]
        S2[Roots/Square roots]
        S3[Superscripts/Subscripts]
        S4[Matrices]
        S5[Binomials]
        S6[Integrals/Sums]
    end
```

### 6. Mermaid Subsystem (`mermaid/`)

Native Rust implementation of Mermaid diagram rendering.

```mermaid
flowchart TB
    SOURCE[Mermaid Source Code] --> PARSER[Parser]
    PARSER --> AST[Diagram AST]
    AST --> LAYOUT[Layout Engine]
    LAYOUT --> POSITIONS[Node Positions]
    POSITIONS --> RENDER[SVG Renderer]
    RENDER --> SVG[SVG Output]
    
    subgraph DiagramTypes["Supported Diagram Types"]
        DT1[Flowchart]
        DT2[Sequence]
        DT3[Class]
        DT4[State]
        DT5[ER Diagram]
    end
    
    PARSER --> DiagramTypes
```

## Data Flow Summary

```mermaid
sequenceDiagram
    participant User
    participant CLI as main.rs
    participant Theme as theme.rs
    participant Font as fonts.rs
    participant Render as renderer.rs
    participant Output as SVG/PNG/PDF
    
    User->>CLI: markie input.md -o output.png
    CLI->>Theme: Load theme (default or file)
    Theme-->>CLI: Theme struct
    CLI->>Font: Initialize font system
    Font-->>CLI: CosmicTextMeasure
    CLI->>Render: Create renderer with theme
    CLI->>Render: render(markdown)
    Render->>Render: Parse Markdown events
    Render->>Font: Measure text (cached)
    Font-->>Render: Text dimensions
    Render->>Render: Generate SVG elements
    Render-->>CLI: SVG string
    CLI->>Output: Convert to PNG
    Output-->>User: output.png
```

## Key Design Decisions

### 1. Pure Rust Implementation

```mermaid
flowchart TB
    TRAD["Traditional: JS Runtime + node_modules"] -->|Heavy| DEPLOY[Deployment]
    MARKIE["Markie: Single Rust Binary"] -->|Lightweight| DEPLOY
```

### 2. Singleton Font System

The font system uses a global singleton with LRU caching to avoid expensive re-initialization:

```mermaid
flowchart TB
    R1[Render Request 1] --> GFS[Global Font System]
    R2[Render Request 2] --> GFS
    R3[Render Request 3] --> GFS
    GFS --> CACHE[Shared LRU Cache]
    CACHE --> HIT{Cache Hit?}
    HIT -->|Yes| FAST[Fast Return]
    HIT -->|No| COMPUTE[Compute & Cache]
    COMPUTE --> FAST
```

### 3. Event-Driven Rendering

Markdown is processed as a stream of events rather than a full AST:

```mermaid
flowchart TB
    MD[Markdown] --> PARSER[Parser]
    PARSER --> E1[Event: Start Heading]
    PARSER --> E2[Event: Text "Hello"]
    PARSER --> E3[Event: End Heading]
    E1 --> RENDER[Renderer]
    E2 --> RENDER
    E3 --> RENDER
    RENDER --> SVG[SVG Output]
```

## File Structure Map

```mermaid
flowchart TB
    MAIN[main.rs] --> RENDERER[renderer.rs]
    RENDERER --> THEME[theme.rs]
    RENDERER --> FONTS[fonts.rs]
    RENDERER --> MATH[math.rs]
    RENDERER --> MERMAID[mermaid/]
    MERMAID --> PARSER[parser.rs]
    MERMAID --> LAYOUT[layout.rs]
    MERMAID --> RENDER[render.rs]
```

---

*Next: [Rendering Pipeline](02-rendering-pipeline.md)*

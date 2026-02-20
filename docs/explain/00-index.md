# Markie Documentation Index

> Welcome to the Markie documentation. This index helps you navigate the detailed explanation documents.

## What is Markie?

Markie is a **pure Rust** command-line tool that converts Markdown documents into beautiful images (SVG, PNG, or PDF). It supports:

- Complete Markdown syntax
- Mermaid diagrams (flowchart, sequence, class, state, ER)
- LaTeX math expressions
- Syntax-highlighted code blocks
- Customizable themes

```mermaid
mindmap
  root((Markie))
    Features
      Markdown to Image
      Mermaid Diagrams
      LaTeX Math
      Syntax Highlighting
    Outputs
      SVG Vector
      PNG Raster
      PDF Document
    Benefits
      Zero Runtime
      Single Binary
      Native Performance
```

## Documentation Overview

Here's how the documentation is organized:

```mermaid
flowchart TB
    subgraph Docs["Documentation Structure"]
        direction TB
        
        ARCH["01 - Architecture Overview"]
        RENDER["02 - Rendering Pipeline"]
        MERMAID["03 - Mermaid Subsystem"]
        THEME["04 - Theme System"]
        OUTPUT["05 - Output Formats"]
        MATH["06 - Math Rendering"]
    end
    
    ARCH --> RENDER
    RENDER --> MERMAID
    RENDER --> THEME
    RENDER --> MATH
    MERMAID --> OUTPUT
    THEME --> OUTPUT
    MATH --> OUTPUT
```

## Document Summaries

### 1. [Architecture Overview](01-architecture-overview.md)

Learn about Markie's overall system design:

- High-level pipeline architecture
- Core components and their responsibilities
- Data flow through the system
- Key design decisions

```mermaid
flowchart TB
    A[Input] --> B[Parse]
    B --> C[Render]
    C --> D[Output]
```

**Who should read:** Anyone wanting to understand how Markie works at a high level.

---

### 2. [Rendering Pipeline](02-rendering-pipeline.md)

Deep dive into the Markdown-to-SVG transformation:

- Markdown parsing with pulldown-cmark
- Event-driven rendering
- Text measurement and layout
- Special block handling (code, tables, images)

```mermaid
flowchart TB
    MD[Markdown] --> EVENTS[Events]
    EVENTS --> STATE[State Machine]
    STATE --> SVG[SVG Elements]
```

**Who should read:** Those interested in the core rendering logic.

---

### 3. [Mermaid Subsystem](03-mermaid-subsystem.md)

Understand how Mermaid diagrams are rendered natively:

- Supported diagram types
- Parsing Mermaid syntax
- Layout algorithms
- SVG generation

```mermaid
flowchart TB
    MERMAID[Mermaid Text] --> AST[AST]
    AST --> LAYOUT[Layout]
    LAYOUT --> SVG[SVG]
```

**Who should read:** Anyone using or extending Mermaid diagram support.

---

### 4. [Theme System](04-theme-system.md)

Learn about visual customization:

- Theme properties
- Alacritty theme format support
- Color mapping logic
- Typography and spacing

```mermaid
flowchart TB
    FILE[Theme File] --> PARSE[Parse]
    PARSE --> THEME[Theme Struct]
    THEME --> RENDER[Apply to Render]
```

**Who should read:** Users wanting to customize output appearance.

---

### 5. [Output Formats](05-output-formats.md)

Explore the three output formats:

- SVG generation
- PNG rasterization
- PDF conversion
- Format-specific options

```mermaid
flowchart TB
    SVG[SVG] --> PNG[PNG]
    SVG --> PDF[PDF]
```

**Who should read:** Those needing specific output format details.

---

### 6. [Math Rendering](06-math-rendering.md)

Understand LaTeX math support:

- LaTeX to MathML conversion
- MathML parsing
- Layout calculation
- SVG generation

```mermaid
flowchart TB
    LATEX[LaTeX] --> MATHML[MathML]
    MATHML --> AST[AST]
    AST --> SVG[SVG]
```

**Who should read:** Users including mathematical formulas in documents.

---

## Quick Navigation by Task

```mermaid
flowchart TB
    TASK{What do you want to do?}
    
    TASK -->|Understand the system| ARCH[Read Architecture Overview]
    TASK -->|Fix a rendering issue| RENDER[Read Rendering Pipeline]
    TASK -->|Add diagram support| MERMAID[Read Mermaid Subsystem]
    TASK -->|Create a theme| THEME[Read Theme System]
    TASK -->|Output to specific format| OUTPUT[Read Output Formats]
    TASK -->|Render math formulas| MATH[Read Math Rendering]
```

## Key Concepts Map

```mermaid
mindmap
  root((Key Concepts))
    Pipeline
      Input → Parse → Render → Output
      Event-driven processing
      Streaming design
    Components
      Parser pulldown-cmark
      Renderer SVG generation
      Font System cosmic-text
      Mermaid Native Rust
    Outputs
      SVG Vector graphics
      PNG Raster with scaling
      PDF Documents
    Customization
      Themes YAML/TOML
      Width control
      Scale factors
```

## Technology Stack

```mermaid
flowchart TB
    subgraph Core["Core Dependencies"]
        PULLDOWN[pulldown-cmark]
        COSMIC[cosmic-text]
        RESVG[resvg + tiny-skia]
    end
    
    subgraph Features["Feature Dependencies"]
        SYNTECT[syntect]
        L2M[latex2mathml]
        S2PDF[svg2pdf]
    end
    
    subgraph Utilities["Utility Dependencies"]
        CLAP[clap]
        SERDE[serde]
        LRU[lru + parking_lot]
    end
    
    Core --> Features
    Features --> Utilities
```

## Getting Started

1. **Read the Architecture Overview** to understand the big picture
2. **Explore specific topics** based on your needs
3. **Check the README** for usage examples

```mermaid
flowchart TB
    START[New to Markie] --> ARCH[Architecture Overview]
    ARCH --> EXPLORE[Explore Specific Topics]
    EXPLORE --> USE[Use Markie Effectively]
```

---

*Happy documenting with Markie!*

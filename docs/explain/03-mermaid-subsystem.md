# The Mermaid Subsystem

> This document explains how Markie natively renders Mermaid diagrams without any JavaScript runtime.

## What is Mermaid?

Mermaid is a text-based diagramming syntax. Instead of drawing diagrams with a mouse, you write them as code. Markie implements a pure Rust parser and renderer for Mermaid diagrams.

```mermaid
flowchart TB
    A[Mermaid Text] --> B[Markie Parser]
    B --> C[Diagram AST]
    C --> D[Layout Engine]
    D --> E[SVG Output]
    
    style A fill:#fff3e0
    style E fill:#e8f5e9
```

## Supported Diagram Types

```mermaid
mindmap
  root((Mermaid Diagrams))
    Flowchart
      TD/TB Top to bottom
      LR Left to right
      Node shapes
      Edge styles
    Sequence
      Participants
      Messages
      Notes
      Control blocks
    Class
      Classes
      Attributes
      Methods
      Relationships
    State
      States
      Transitions
      Composite states
    ER
      Entities
      Relationships
      Attributes
```

## Architecture Overview

The Mermaid subsystem is organized into four main modules:

```mermaid
flowchart TB
    subgraph Input["Input"]
        SRC[Mermaid Source Code]
    end
    
    subgraph Parser["parser.rs"]
        DETECT[Detect Diagram Type]
        PARSE[Parse to AST]
        AST[Abstract Syntax Tree]
    end
    
    subgraph Layout["layout.rs"]
        ENGINE[Layout Engine]
        POS[Calculate Positions]
        BBOX[Bounding Box]
    end
    
    subgraph Render["render.rs + flowchart.rs"]
        SVG_GEN[SVG Generator]
        STYLE[Apply Styles]
        OUTPUT[SVG Fragment]
    end
    
    SRC --> DETECT
    DETECT --> PARSE
    PARSE --> AST
    AST --> ENGINE
    ENGINE --> POS
    POS --> BBOX
    BBOX --> SVG_GEN
    SVG_GEN --> STYLE
    STYLE --> OUTPUT
```

## Stage 1: Parsing (parser.rs)

The parser converts Mermaid text into structured data.

```mermaid
flowchart TB
    INPUT[Mermaid Source] --> FIRST[Read First Line]
    FIRST --> TYPE{Diagram Type?}
    
    TYPE -->|flowchart/graph| FLOW[parse_flowchart]
    TYPE -->|sequenceDiagram| SEQ[parse_sequence]
    TYPE -->|classDiagram| CLASS[parse_class]
    TYPE -->|stateDiagram| STATE[parse_state]
    TYPE -->|erDiagram| ER[parse_er]
    
    FLOW --> AST[Diagram AST]
    SEQ --> AST
    CLASS --> AST
    STATE --> AST
    ER --> AST
```

### Flowchart Parsing

```mermaid
flowchart TB
    FC_INPUT["Flowchart Source"]
    
    FC_INPUT --> LINES[Split into Lines]
    LINES --> DIR[Parse Direction: TD]
    
    DIR --> NODE_A["Node A: Start, Rect"]
    DIR --> NODE_B["Node B: Choice, Rhombus"]
    DIR --> NODE_C["Node C: Continue, Rect"]
    
    DIR --> EDGE_1["Edge: A → B"]
    DIR --> EDGE_2["Edge: B → C, label=Yes"]
    
    NODE_A --> AST[Flowchart AST]
    NODE_B --> AST
    NODE_C --> AST
    EDGE_1 --> AST
    EDGE_2 --> AST
```

### Node Shape Detection

```mermaid
flowchart TB
    NODE["A[Label]" or "A(Label)" or "A{Label}"]
    
    NODE --> PATTERN{Match Pattern}
    
    PATTERN -->|"A[Label]"| RECT[Rect Shape]
    PATTERN -->|"A(Label)"| ROUNDED[Rounded Rect]
    PATTERN -->|"A{Label}"| RHOMBUS[Rhombus/Diamond]
    PATTERN -->|"A((Label))"| CIRCLE[Circle]
    PATTERN -->|"A[[Label]]"| SUBROUTINE[Subroutine]
    PATTERN -->|"A[(Label)]"| CYLINDER[Cylinder]
    PATTERN -->|"A([Label])"| STADIUM[Stadium]
    PATTERN -->|"A[/Label/]"| PARA[Parallelogram]
    PATTERN -->|"A{{Label}}"| HEX[Hexagon]
    
    RECT --> RESULT[NodeInfo]
    ROUNDED --> RESULT
    RHOMBUS --> RESULT
    CIRCLE --> RESULT
    SUBROUTINE --> RESULT
    CYLINDER --> RESULT
    STADIUM --> RESULT
    PARA --> RESULT
    HEX --> RESULT
```

### Edge Style Detection

```mermaid
flowchart TB
    EDGE["A --> B" or "A -.-> B" or "A ==> B"]
    
    EDGE --> PATTERNS{Match Pattern}
    
    PATTERNS -->|"-->"| SOLID[Solid Arrow]
    PATTERNS -->|"---"| LINE[Solid Line]
    PATTERNS -->|"-.->"| DOTTED[Dotted Arrow]
    PATTERNS -->|"-.-"| DOT_LINE[Dotted Line]
    PATTERNS -->|"==>"| THICK[Thick Arrow]
    PATTERNS -->|"==="<| THICK_REV[Thick Arrow Reverse]
    PATTERNS -->|"--o"| CIRCLE[Circle End]
    PATTERNS -->|"--x"| CROSS[Cross End]
    PATTERNS -->|"<-->"| BIDI[Bidirectional]
    
    SOLID --> STYLE[EdgeStyle]
    LINE --> STYLE
    DOTTED --> STYLE
    DOT_LINE --> STYLE
    THICK --> STYLE
    THICK_REV --> STYLE
    CIRCLE --> STYLE
    CROSS --> STYLE
    BIDI --> STYLE
```

### Sequence Diagram Parsing

```mermaid
flowchart TB
    SEQ_INPUT["Sequence Source"]
    
    SEQ_INPUT --> PARTICIPANTS[Parse Participants]
    PARTICIPANTS --> P1[Participant: Alice]
    PARTICIPANTS --> P2[Participant: Bob]
    
    P1 --> ELEMENTS[Parse Elements]
    P2 --> ELEMENTS
    
    ELEMENTS --> MSG1["Message: Alice → Bob"]
    ELEMENTS --> MSG2["Message: Bob → Alice"]
    
    MSG1 --> SEQ_AST[SequenceDiagram AST]
    MSG2 --> SEQ_AST
```

### Control Block Parsing

Sequence diagrams support nested control structures:

```mermaid
flowchart TB
    BLOCK["Control Block Source"]
    
    BLOCK --> TYPE_BLOCK[Detect Block Type: loop]
    TYPE_BLOCK --> LABEL[Extract Label: "Every minute"]
    LABEL --> CHILDREN[Parse Child Elements Recursively]
    
    CHILDREN --> C1[Message: A → B "Ping"]
    CHILDREN --> C2[Message: B → A "Pong"]
    
    C1 --> BLOCK_AST[SequenceBlock]
    C2 --> BLOCK_AST
```

### Supported Control Blocks

```mermaid
flowchart TB
    CTRL[Control Block Types]
    
    CTRL --> ALT["alt/opt"]
    CTRL --> LOOP["loop"]
    CTRL --> PAR["par"]
    CTRL --> CRITICAL["critical"]
    
    ALT --> ELSE["else clause"]
```

## Stage 2: Layout (layout.rs)

The layout engine calculates positions for all diagram elements.

```mermaid
flowchart TB
    AST[Diagram AST] --> ENGINE[LayoutEngine]
    ENGINE --> MEASURE[Measure Text Sizes]
    MEASURE --> POSITIONS[Calculate Positions]
    POSITIONS --> BBOX[Calculate Bounding Box]
    
    subgraph LayoutEngine["LayoutEngine State"]
        FONT[Font Size: 13px]
        SPACING_X[Node Spacing X: 48px]
        SPACING_Y[Node Spacing Y: 52px]
        PADDING_H[Horizontal Padding: 16px]
        PADDING_V[Vertical Padding: 10px]
    end
```

### Flowchart Layout Algorithm

The flowchart uses a layered (hierarchical) layout:

```mermaid
flowchart TB
    FC[Flowchart] --> LAYERS[Assign Nodes to Layers]
    LAYERS --> ORDER[Order Nodes Within Layers]
    ORDER --> BARYCENTER[Apply Barycenter Ordering]
    BARYCENTER --> POSITION[Calculate Final Positions]
    
    subgraph LayerAssignment["Layer Assignment"]
        direction TB
        L1[Layer 0: Source Nodes]
        L2[Layer 1: After 1 Edge]
        L3[Layer 2: After 2 Edges]
        LN[...]
    end
    
    LAYERS --> LayerAssignment
```

### Layer Assignment Process

```mermaid
flowchart TB
    NODES[All Nodes] --> TOPO[Topological Sort]
    TOPO --> ASSIGN[Assign Layer Numbers]
    
    ASSIGN --> RULE1[Rule 1: Source nodes = Layer 0]
    ASSIGN --> RULE2[Rule 2: Node layer = max of incoming edge layers + 1]
    ASSIGN --> RULE3[Rule 3: Min-length constraints honored]
    
    RULE1 --> RESULT[Node → Layer Mapping]
    RULE2 --> RESULT
    RULE3 --> RESULT
```

### Barycenter Ordering

To reduce edge crossings, nodes are ordered using the barycenter heuristic:

```mermaid
flowchart TB
    LAYER[Layer to Order] --> CALC[Calculate Barycenter for Each Node]
    
    CALC --> FORMULA["barycenter(node) = avg(position of connected nodes in previous layer)"]
    
    FORMULA --> SORT[Sort by Barycenter Value]
    SORT --> ITERATE[Repeat for Each Layer]
    ITERATE --> CONVERGE{Converged?}
    CONVERGE -->|No| CALC
    CONVERGE -->|Yes| FINAL[Final Node Order]
```

### Sequence Diagram Layout

```mermaid
flowchart TB
    SEQ[Sequence Diagram] --> PARTICIPANTS[Layout Participants]
    PARTICIPANTS --> CENTERS[Calculate Center Positions]
    CENTERS --> REQUIREMENTS[Collect Pair Requirements]
    REQUIREMENTS --> ADJUST[Adjust Spacing for Labels]
    ADJUST --> MESSAGES[Layout Messages Vertically]
    
    subgraph ParticipantLayout["Participant Layout"]
        direction LR
        P1[Participant 1] --- P2[Participant 2] --- P3[Participant 3]
        
        note1[Center 1]
        note2[Center 2]
        note3[Center 3]
    end
    
    PARTICIPANTS --> ParticipantLayout
```

### Class Diagram Layout

```mermaid
flowchart TB
    CLASS[Class Diagram] --> SIZE[Calculate Class Box Sizes]
    SIZE --> DEPS[Build Dependency Graph]
    DEPS --> LAYERED[Apply Layered Layout]
    LAYERED --> RELATIONS[Route Relations]
    
    subgraph ClassBox["Class Box Size Calculation"]
        HEADER[Header: Class Name]
        ATTRS[Attributes Section]
        METHODS[Methods Section]
        SEP[Separators]
    end
    
    SIZE --> ClassBox
```

## Stage 3: Rendering (render.rs + flowchart.rs)

The renderer converts positioned elements to SVG.

```mermaid
flowchart TB
    POSITIONS[Node Positions] --> STYLE[Apply DiagramStyle]
    STYLE --> NODES[Render Nodes]
    NODES --> EDGES[Render Edges]
    EDGES --> LABELS[Render Labels]
    LABELS --> FRAGMENT[SVG Fragment]
    
    subgraph DiagramStyle["DiagramStyle Configuration"]
        FILL[Node Fill Color]
        STROKE[Node Stroke Color]
        TEXT[Text Color]
        EDGE[Edge Color]
        FONT[Font Settings]
    end
    
    STYLE --> DiagramStyle
```

### Flowchart Node Rendering

```mermaid
flowchart TB
    NODE[FlowchartNode] --> SHAPE{Shape Type?}
    
    SHAPE -->|Rect| RECT[Draw Rectangle]
    SHAPE -->|RoundedRect| ROUND[Draw Rounded Rectangle]
    SHAPE -->|Rhombus| RHOMBUS[Draw Diamond Path]
    SHAPE -->|Circle| CIRCLE[Draw Circle]
    SHAPE -->|Cylinder| CYLINDER[Draw Ellipse + Rectangle]
    SHAPE -->|Hexagon| HEX[Draw Hexagon Path]
    SHAPE -->|Stadium| STADIUM[Draw Stadium Shape]
    
    RECT --> TEXT[Draw Label Text]
    ROUND --> TEXT
    RHOMBUS --> TEXT
    CIRCLE --> TEXT
    CYLINDER --> TEXT
    HEX --> TEXT
    STADIUM --> TEXT
    
    TEXT --> GROUP[Wrap in SVG Group]
```

### Edge Path Calculation

```mermaid
flowchart TB
    EDGE[Edge: A → B] --> FROM[Get Node A Center]
    FROM --> TO[Get Node B Center]
    TO --> DIR{Same Row/Column?}
    
    DIR -->|Yes| STRAIGHT[Draw Straight Line]
    DIR -->|No| ROUTE[Calculate Route Points]
    
    ROUTE --> ORTHOGONAL[Orthogonal Routing]
    ORTHOGONAL --> SEGMENTS[Create Path Segments]
    SEGMENTS --> PATH[Build SVG Path]
    
    PATH --> ARROW{Arrow Type?}
    ARROW -->|Arrow| ARROW_HEAD[Add Arrow Marker]
    ARROW -->|Circle| CIRCLE_END[Add Circle End]
    ARROW -->|Cross| CROSS_END[Add Cross End]
    ARROW -->|None| NO_END[No End Marker]
    
    ARROW_HEAD --> FINAL[Final Edge SVG]
    CIRCLE_END --> FINAL
    CROSS_END --> FINAL
    NO_END --> FINAL
```

### Edge Label Placement

```mermaid
flowchart TB
    LABEL[Edge Label] --> MID[Find Edge Midpoint]
    MID --> OFFSET[Calculate Offset]
    OFFSET --> COLLISION{Collision?}
    
    COLLISION -->|Yes| ADJUST[Adjust Position]
    COLLISION -->|No| PLACE[Place Label]
    
    ADJUST --> PLACE
    PLACE --> BG[Draw Background]
    BG --> TEXT[Draw Text]
    
    subgraph CollisionCheck["Collision Detection"]
        NODES[Check Against Nodes]
        OTHER[Check Against Other Labels]
    end
    
    COLLISION --> CollisionCheck
```

### Sequence Diagram Rendering

```mermaid
flowchart TB
    SEQ[Sequence Diagram] --> PART[Render Participants]
    PART --> LIFELINE[Draw Lifelines]
    LIFELINE --> MESSAGES[Render Messages]
    MESSAGES --> BLOCKS[Render Control Blocks]
    
    subgraph ParticipantBox["Participant Box"]
        RECT[Rectangle Box]
        NAME[Participant Name]
    end
    
    PART --> ParticipantBox
    
    subgraph MessageLine["Message Line"]
        ARROW[Arrow Type]
        LABEL_MSG[Message Label]
        DASHED[Dotted for Reply]
    end
    
    MESSAGES --> MessageLine
```

### Control Block Rendering

```mermaid
flowchart TB
    BLOCK[Sequence Block] --> RECT_B[Draw Bounding Rectangle]
    RECT_B --> TITLE[Render Block Title]
    TITLE --> CONTENTS[Render Contents]
    
    BLOCK --> ELSE{Has Else Branch?}
    ELSE -->|Yes| SEPARATOR[Draw Dashed Separator]
    SEPARATOR --> ELSE_CONTENT[Render Else Contents]
    
    ELSE_CONTENT --> NESTED{Nested Blocks?}
    CONTENTS --> NESTED
    
    NESTED -->|Yes| RECURSE[Recursively Render]
    NESTED -->|No| DONE[Block Complete]
    RECURSE --> DONE
```

## Diagram Style System

Colors are automatically derived from the theme:

```mermaid
flowchart TB
    THEME[Theme Colors] --> CONTRAST[Calculate Contrast]
    CONTRAST --> MIX[Mix Colors]
    MIX --> STYLE[DiagramStyle]
    
    subgraph ColorDerivation["Color Derivation"]
        BG[Background Color]
        TEXT[Text Color]
        CODE[Code Background]
        
        BG --> NODE_FILL[Node Fill: Mix 3%]
        CODE --> NODE_STROKE[Node Stroke: Mix 20%]
        CODE --> EDGE_COLOR[Edge Color: Mix 30%]
        CODE --> LABEL_COLOR[Label Color: Mix 60%]
    end
    
    THEME --> ColorDerivation
```

## Complete Example: Flowchart

Let's trace through rendering a complete flowchart:

```mermaid
sequenceDiagram
    participant SRC as Source
    participant P as Parser
    participant L as Layout
    participant R as Renderer
    participant SVG as SVG Output
    
    SRC->>P: flowchart TD; A[Start] --> B{Choice}; B --> C[End]
    
    Note over P: Parse direction: TopDown
    Note over P: Parse node A: label="Start", shape=Rect
    Note over P: Parse node B: label="Choice", shape=Rhombus
    Note over P: Parse node C: label="End", shape=Rect
    Note over P: Parse edge A → B
    Note over P: Parse edge B → C
    
    P->>L: Flowchart AST
    
    Note over L: Assign layers: A=0, B=1, C=2
    Note over L: Calculate node sizes
    Note over L: Position nodes vertically
    
    L->>R: Positions & Bounding Box
    
    Note over R: Draw node A rect at (50, 20)
    Note over R: Draw node B diamond at (50, 100)
    Note over R: Draw node C rect at (50, 180)
    Note over R: Draw edge A → B
    Note over R: Draw edge B → C
    Note over R: Draw labels "Start", "Choice", "End"
    
    R->>SVG: SVG Fragment
    
    Note over SVG: <g><rect...><path...><text...></g>
```

## Error Handling

```mermaid
flowchart TB
    PARSE[Parse Operation] --> VALID{Valid Syntax?}
    
    VALID -->|Yes| CONTINUE[Continue Processing]
    VALID -->|No| ERROR[Parse Error]
    
    ERROR --> TYPE{Error Type}
    TYPE -->|Unknown Diagram| UNKNOWN["Unknown diagram type"]
    TYPE -->|Invalid Syntax| INVALID["Invalid syntax: ..."]
    TYPE -->|Missing Element| MISSING["Missing required element"]
    
    UNKNOWN --> FALLBACK[Fallback to Flowchart]
    INVALID --> MSG[Return Error Message]
    MISSING --> MSG
    
    FALLBACK --> CONTINUE
```

## Performance Considerations

```mermaid
flowchart TB
    DIAGRAM[Diagram] --> COMPLEXITY{Complexity}
    
    COMPLEXITY -->|Simple| FAST[Fast Path]
    COMPLEXITY -->|Medium| NORMAL[Normal Path]
    COMPLEXITY -->|Complex| OPTIMIZE[Optimized Path]
    
    subgraph Optimizations["Optimizations Applied"]
        CACHE[Text Measurement Caching]
        SPARSE[Sparse Node Spacing]
        COLLISION[Collision-Aware Labels]
    end
    
    NORMAL --> Optimizations
    OPTIMIZE --> Optimizations
```

---

*Previous: [Rendering Pipeline](02-rendering-pipeline.md)*
*Next: [Theme System](04-theme-system.md)*

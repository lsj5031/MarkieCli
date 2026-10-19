# Everyday syntax compatibility

Before compatibility content: native documents keep formatting and relationships together.

> [!NOTE]
> Review the source and the rendered preview together.

> [!WARNING]
> Confirm the recipient before sharing an export.

| Review item | Status | Next step |
| --- | --- | --- |
| **Document** and *diagrams* | ~~Draft~~ Reviewed | Run `markie --help` |
| [Review packet](https://example.com/review) | <u>Signed</u><br>Ready | <span style="color:#c0392b">Red notice</span> then plain text |

## Chained and grouped dependencies

```mermaid
flowchart LR
    A[Draft] --> B[Review] --> C[Publish];
    D[Legal] & E[Security] --> B
```

## Implicit participants and activation shorthand

```mermaid
sequenceDiagram
    Author->>+Renderer: Render source
    Renderer-->>-Author: Deliver preview
```

## Classes, multiplicities and member classifiers

```mermaid
classDiagram
    Renderable <|.. Document
    Document "1" --> "0..*" Export : produces
    class Document {
        +render() bool
        +fromSource() Document$
        +validate() bool*
    }
```

After compatibility content: every section must remain visible at both document widths.

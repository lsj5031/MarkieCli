# Flowchart quality

Before the diagram: this paragraph must stay within the document margin.

```mermaid
%% A leading comment must not turn LR into TD or create a header node.
flowchart LR
    A[Receive request] -->|validate| B[Check policy<br>Second line<br/>Third line<br />Fourth line]
    B --> C[Publish result]
```

## Long labels and grouping

```mermaid
flowchart TD
    subgraph Review
    A[An intentionally long review label that exercises measured node width] --> B[承認待ち 日本語 中文]
    end
    B -->|approved| C[Release]
```

## Cycle and self-loop

```mermaid
flowchart LR
    A[Queued] --> B[Running]
    B -->|retry| A
    B -->|heartbeat| B
    B --> C[Finished]
```

After the diagram: the final paragraph must remain visible.

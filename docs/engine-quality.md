# Engine support and quality evidence

Markie renders a useful subset of Markdown and five Mermaid families in Rust.
It does not implement the full Mermaid language or browser HTML/CSS layout.
Accepted input is not a guarantee of unclipped text or collision-free routing.

## Support boundary

| Area | Intended support | Boundary |
| --- | --- | --- |
| Markdown | Headings, paragraphs, emphasis, code, lists/tasks, quotes, links, tables, footnotes, definition lists, images | Links are colored text. Metadata is ignored. Wide tables and long unbroken content need visual review at the intended width. |
| HTML | Basic inline styling, superscript/subscript, underline, mark, font color | HTML blocks display as code; arbitrary CSS and rich HTML layouts are outside scope. |
| Math | Inline/display LaTeX-style expressions, including roots, fractions, binomials and matrices | A subset translated through MathML, not a TeX engine. Inspect complex expressions. |
| Flowchart | `flowchart`/`graph`, TD/TB/BT/LR/RL, node shapes, labeled edges, solid/dotted/thick and arrow variants, flat subgraphs | Nested subgraphs are explicitly rejected. `<br>`, `<br/>`, `<br />` are node-label line breaks. Labels do not gain automatic word wrapping just because document width is narrow. |
| Sequence | Participants, messages/replies, notes, activation/deactivation, `alt`/`opt`/`loop`/`par`/`critical` blocks with `else`/`end` | This names implemented constructs, not full Mermaid compatibility or a layout guarantee for every nesting combination. |
| Class | Classes, attributes/methods, relationships | Use the examples in the smoke fixture as the minimum exercised subset. |
| State | States, transitions, composite children and notes | Basic transitions are in the corpus; complex composite layout is not certified by it. |
| ER | Entities, attributes, cardinality relationships | Basic entity relationships are in the corpus; this is not a schema validator. |

Unknown families (for example `pie`, `gantt`, `journey`) produce an unsupported
diagram error. An unsupported Mermaid block fails the document render; the CLI
reports its Markdown fence line. An empty/comment-only Mermaid fence renders an
empty diagram. Rejection of unknown families and nested subgraphs does **not**
mean every unsupported statement inside a recognized family is rejected:
parsing remains permissive in places. Do not depend on Mermaid `init` directives
to configure theme/layout; `%%` comment lines are skipped. Use Markie's theme CLI.

## Repeatable validation

From the repository root, with Rust/Cargo, a C linker, Bash and Python 3 available:

```bash
bash scripts/smoke-test.sh target/engine-quality
```

This reuses the existing smoke renderer: it runs `cargo test`, builds the current
binary, renders the original math/five-family fixtures and the two quality
documents to SVG, PNG and PDF. Quality documents render at **640px and 1200px**;
PNG defaults to 2× scale. `THEME_FILE` and `PNG_SCALE` retain their existing
overrides. It never updates committed baselines. Use a fresh output directory
when comparing revisions, to avoid confusing old artifacts with the current run.

| Input | Evidence |
| --- | --- |
| `tests/smoke/smoke_mermaid.md` | Flowchart, sequence, class, state and ER; includes a retry cycle |
| `tests/smoke/smoke_math.md` | Roots, binomials, matrices and mixed expression |
| `tests/smoke/quality/flowchart.md` | Leading comment + LR; all three HTML break spellings; long/CJK labels; flat group; cycle and self-loop; surrounding paragraphs |
| `tests/smoke/quality/document.md` | Wide four-column table; CJK prose; inline/display math; highlighted code; long LR graph; surrounding document width |
| `tests/smoke/quality/unsupported/*.md` | Nested subgraph and unknown-family rejection with source location |

`scripts/check-quality.py` checks parseable SVG, representative visible text
content, LR node order/alignment after a leading comment, absence of literal
break tags/header nodes, PNG signature and scaled dimensions, and PDF signature.
The shell checks require nonzero exit status and the expected diagnostics for
unsupported inputs. Text preservation in SVG does not prove that fonts contain
the glyphs, that edges connect correctly, or that labels fit. The checker does
not certify general geometry, all relationships, math correctness or PDF content.

If Poppler's `pdftoppm` is available, the same command also renders page 1 of each
PDF to `*-pdf.png`. Review the PDF itself if pagination changes. CI installs
Poppler. Open each SVG, PNG and PDF/preview; check missing glyphs, clipping,
node/edge-label overlap, arrow direction, table text wrapping, group membership,
document margins and text after diagrams. Compare both widths at readable zoom.

### Fonts and environment

Historical smoke/demo baseline jobs use Ubuntu 24.04 with `fonts-dejavu-core`
and Rust **1.97.1**. The quality evidence step adds `fontconfig`,
`fonts-noto-cjk` and `poppler-utils` **after** checking the old smoke baselines.
This gives CJK a deliberate font source without changing that baseline contract.
These are distro packages, not byte-pinned font files; the CI container tag and
apt repositories can change. Review the recorded inventory when comparing runs.

On a Debian/Ubuntu development machine the relevant font/preview packages are:

```bash
sudo apt-get install fontconfig fonts-dejavu-core fonts-noto-cjk poppler-utils
```

The script writes `environment.txt` with the Rust version, binary/theme hashes,
scale, fontconfig family matches and installed font inventory. Fontconfig matches
are context, not proof of the actual font used by each glyph: measurement uses
cosmic-text; PNG/PDF load system fonts and a working-directory `fonts/` directory;
SVG viewers can choose their own fonts. Run from the repository root and keep the
font environment consistent. Missing CJK fonts can produce tofu even when the
SVG retains every Unicode character.

### Baselines and CI review

The existing checks remain available:

```bash
bash scripts/check-smoke.sh target/smoke-check
bash scripts/make-demo.sh target/demo-check
bash scripts/check-demos.sh target/demo-check
```

Smoke/demo SVG comparisons strip coordinates, transforms, points and path data,
but retain text, colors, structure and root viewBox. A passing check therefore
does **not** establish correct edge routing, label placement or unclipped output.
Demo PNG checks compare dimensions; PDF checks inspect a signature, not content.

CI retains full, unnormalized smoke/quality outputs, source inputs, diagnostic
logs, environment inventory and PDF previews in `engine-quality-<commit>`;
regenerated demos are in `demo-evidence-<commit>`. Retention is 14 days. Download
and inspect artifacts even when checks pass, especially for layout changes.
Failed steps upload whatever was produced. No new binary golden files are kept.

If a baseline needs changing, generate into a scratch directory first, inspect
the full SVG and raster/PDF output, explain the intended difference, then copy
only reviewed assets. Do not run the default `make-smoke.sh` or `make-demo.sh`
over committed files merely to make CI green.

## Bounded feature-trimming assessment

These are recommendations, not removals. No usage telemetry or dependency-size
benchmark was collected, so there is no evidence here for deleting an entire
diagram family or export format.

| Recommendation | Evidence and user impact | Compatibility cost |
| --- | --- | --- |
| Advertise a Mermaid subset and example-backed support rather than broad "advanced syntax" compatibility | The parser recognizes five families, skips comments/config directives and has permissive paths inside known families. Users need a clear boundary before migrating Mermaid documents. | Documentation-only qualification does not break inputs. Adding strict rejection later may break documents that currently render partial output; needs a separate decision. |
| Keep nested subgraphs outside supported syntax | The current AST stores flat groups; silently flattening loses membership. Explicit failure avoids publishing misleading diagrams. | Existing documents with nested groups stop rendering and must be flattened or rendered elsewhere. This is a deliberate compatibility change in the parser work. |
| Qualify "complete Markdown" and "production-ready PDF" advertising | HTML blocks are code, metadata is ignored, and PDF uses text-as-paths. The corpus is a small evidence set, not broad document conformance. | No rendering change. Users needing searchable/selectable PDF text or browser HTML layout should choose another workflow. |
| Keep existing formats and five families for now | Existing smoke fixtures already exercise them; no evidence establishes that one is unused or disproportionately expensive. | Removing one would invalidate real existing examples and CLI workflows. Require usage and maintenance evidence before proposing deletion. |

## Reviewed findings and observed limits

The three source-review findings were checked against the starting revision
and the parser work accompanying this corpus:

| Finding | Result and evidence |
| --- | --- |
| Leading comments lose flowchart direction | **Reproduced and fixed.** The first noncomment header must supply direction and must not become a node. The CLI corpus checks LR order/alignment after a comment; parser regression tests cover all five direction spellings and both header aliases. |
| Nested subgraphs lose membership | **Reproduced; explicitly rejected.** The flat AST cannot represent nested ownership. The negative CLI fixture now fails with both group names and the Markdown fence location rather than silently losing the outer group. Flat group rendering remains in the positive corpus. |
| HTML break tags measured differently from drawing | **Dismissed for CLI input.** The starting parser already calls `normalize_flowchart_label` from both `extract_node_info` and `parse_node_definition`, replacing all three break spellings with newlines before layout. Baseline/current CLI output agrees; there is no user-visible multiline fix. The corpus retains four-line coverage so the actual input path stays exercised. |

Local review used the system font inventory recorded in `environment.txt`
(Noto Sans family match, DejaVu Sans Mono, CJK-capable system fallback) and
Rust 1.98.1, rather than the CI font/toolchain environment above. Native PNGs
and Poppler PDF previews were inspected; SVG content and selected coordinates
were checked directly. The media viewer cannot display SVG directly, so native
PNG exports provided the raster view of the renderer's SVG content. Browser SVG
font substitution and the CI artifact renders still need their own review.

Observed results, not a general layout certification:

- All four multiline flowchart lines, long/CJK node labels and surrounding
  paragraphs are visible at both widths. The basic five-family fixture and math
  PDF preview preserve their displayed content without observed clipping.
- In `quality_flowchart_*.svg`, the Queued→Running and Running→Queued routes
  occupy the same segment in reverse. The `retry` direction is visually
  ambiguous; a retained label and passing check do not prove cycle routing.
  The heartbeat label is close to the self-loop path above Running.
- The `approved` label lies on the dashed Review group border at both widths,
  obscuring the boundary where the edge leaves the group.
- The mixed document stays within margins, and its final sentence and CJK prose
  remain visible. At 640px the table splits Component, Collector, Reconciler and
  Publisher into short fragments; even at 1200px Component wraps its final letter.
  Word spacing in narrow prose is visibly loose. Long LR diagrams scale down
  to fit the document, leaving small label text instead of wrapping node labels.
- State-transition and ER-cardinality notation need closer semantic/geometry
  review beyond this corpus. Their basic labels are retained, but the content
  assertions do not validate every arrowhead, relationship or cardinality.

These limitations are retained as evidence rather than hidden by normalization
or expanded into a routing/table rewrite. For narrow documents, prefer shorter
labels, explicit node line breaks, fewer table columns or a wider output. That
does not resolve coincident reverse edges or group-border label collisions.

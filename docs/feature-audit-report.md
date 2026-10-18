# MarkieCli feature audit

Status: complete. The canonical story inventory and live status are in
[`feature-audit.csv`](feature-audit.csv). This report records decisions and final
results; it is not a second progress tracker.

Live dashboard:
<https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/>

The dashboard is read-only and refreshes every 2.5 seconds. It provides a CSV
download, story search and filters, preserved error/fix/retest history, evidence
links, and separate baseline and post-fix progress. It runs on port 8766 on the
audit runner; the OpenTunnel route forwards only that service.

## Scope and method

The audit covers the user-visible behavior implemented by the Markdown renderer,
the five native Mermaid families, the main and standalone command-line tools,
themes, math, fonts, export formats, and installation interfaces. Stories include
normal use, meaningful variations, invalid inputs, and documented unsupported
constructs. Expected behavior is grounded in source and user intent; an observed
defect is not accepted as the expected result.

Three independently owned runner threads inventory and test the product. They
record initial results before production fixes, preserve defect evidence, and
repeat every story after fixes. A fourth thread owns the dashboard and canonical
spreadsheet. The coordinator reviews coverage, integrates changes, and produces
the final report and showcase. Feature tests are executed by the worker threads.

Passing content checks do not establish visual correctness. Relevant stories
also require inspection of rendered outputs, including narrow layouts, themes,
Unicode text, and PDF previews. Unsupported behavior must be distinguished from
untested behavior and from a confirmed defect.

The original issue's reliability-first direction is supported by this audit.
Leading comments losing flowchart direction and nested groups losing membership
were reproduced and corrected (nested groups now fail explicitly). The proposed
`<br>` measurement mismatch was dismissed for real CLI input: the parser already
normalized all three supported spellings before layout. The regression corpus
keeps that behavior covered without claiming a nonexistent fix.

## Product boundary decisions

- Describe Mermaid as a supported subset with explicit examples and diagnostics.
- Keep nested flowchart groups unsupported until hierarchy can be represented
  correctly. Silent membership loss is unacceptable.
- Keep all five existing diagram families and SVG/PNG/PDF exports. There is no
  usage or maintenance evidence justifying removal of an entire family or format.
- Keep useful inline formatting and the standalone Mermaid workflow. Bound the
  HTML/CSS compatibility promise instead of expanding it into browser layout.
- Select showcase features only after their stories pass post-fix validation.
  Demo clips must use real renderer outputs and reproducible commands.

## Findings, fixes, and post-fix results

The initial inventory contains 567 stories: 137 Markdown, 223 Mermaid, and
207 CLI/export/theme/math/font/installation stories. The latter includes
16 additional library-interface contracts identified during testing.

Baseline results, before production fixes:

| Workstream | Passed | Failed | Intentional unsupported boundary |
| --- | ---: | ---: | ---: |
| Markdown | 94 | 34 | 9 |
| Mermaid | 109 | 91 | 23 |
| CLI, exports, themes, math, fonts, installation | 177 | 30 | 0 |

Failed stories are not distinct bug counts: one root cause can affect several
stories. Incorrect test assumptions are also retained in history and explicitly
corrected. In particular, empty Mermaid input intentionally succeeds, the
standalone canvas uses the contrast-adjusted diagram palette, and Markdown may
preserve malformed math delimiters as ordinary text. Those behaviors were not
changed to satisfy an incorrect assertion.

Confirmed defect groups entering the fix phase:

- Markdown: long-token/CJK and inline-code overflow; nonbreaking-space handling;
  missing list and footnote markers; narrow-table cell overflow and misplaced
  table images; inline HTML size, precedence and scope errors; missing image
  alt text and diagnostics; image symlinks escaping the input directory; quote
  spacing; unusable widths; retained state across renderer reuse.
- Mermaid: flowchart endpoint and self-loop marker direction, coincident
  reverse/parallel routes and label clearance; class relationship marker
  semantics; state alias/composite/sentinel data loss and transition direction;
  ER cardinality placement, missing combinations and lost dashed styles.
- CLI and supporting engines: invalid standalone padding and palette colors;
  XML attribute injection through palettes; corrupted Unicode math, lost text
  spaces, script/table/alignment/delimiter defects and silent math errors;
  missing-font panic; standalone binary omitted from release archives; shell
  environment overrides applied to the wrong side of the install pipeline.

Two additional public-math regressions discovered during fixes bring the final
inventory to 569 stories. CLI-208 preserves the original direct/Markdown math
attribute-injection failure. CLI-209 preserves the escaped-underscore script
regression introduced by the converter adapter, together with its correction.
These supplemental failures do not rewrite the original 567-story baseline.

### Markdown fixes

| Stories | Correction and observable result |
| --- | --- |
| MD-013/014/016/028 | Long words, CJK and inline code wrap within the available width; nonbreaking pairs retain their semantics. Prose, inline code, code blocks and tables split at Unicode grapheme boundaries. Public mocked-width cases force discriminating combining/ZWJ boundaries rather than relying only on host-font behavior. |
| MD-038–042/051/072 | Lists and footnote definitions emit their marker with the first actual glyph, including single-word items and nested containers. |
| MD-049/050 | Following content clears the quote border; nested quotes preserve indentation and clearance. |
| MD-064/069 | Dense tables reduce font and padding together, preserve complete cell content, and render image alt text inside the cell instead of placing an image into document flow. Extremely dense tables remain small. |
| MD-085/094/096–098 | Inline HTML recognizes harmless formatting attributes, treats px sizes relative to body size, and restores nested style scopes/precedence correctly. This remains an inline subset. |
| MD-113–118/122–124/128 | Rejected/missing images retain readable alt content and diagnostics. Canonicalized local paths prevent symlink escape; unsupported image forms remain boundaries rather than advertised image support. |
| MD-134–137 | Public renderer construction rejects nonfinite/unusable width and invalid metrics. Reusing a renderer resets document state. |
| Supplemental public contracts | Theme-derived SVG attributes are XML-escaped without banning valid named/rgb CSS colors. Nonfinite table-header opacity is rejected; finite values are clamped to 0–1 at emission. |

### Mermaid fixes

| Stories | Correction and observable result |
| --- | --- |
| MER-019/024/051–084 | Route endpoint angles come from nonzero segments, retaining reverse/RL heads and circle/cross/bidirectional tails. |
| MER-094–102 | Parallel/reverse edges use separate routes. Labels clear short marker segments, group borders and previously placed labels, then paint above route lines on the diagram surface. Self-loops use shape-clipped endpoints and tangent-aligned markers; curve/label extents contribute to the canvas. Later node declarations update the existing identity. The unchanged showcase branch/cycle graph exposed the final overlap and paint-order fixes. |
| MER-111–118/129/135 | Sequence line dash and head semantics remain independent, including self-messages; nested activations use distinct offsets. Notes and self-message labels contribute to bounds. Nested control labels reserve vertical space. |
| MER-145/150–153 | Prefix stereotypes affect class styling; markerless associations/dependencies remain markerless, and directed relationships point at the correct target. |
| MER-161–182 | State transitions derive heads from final route direction; aliases survive references; self-transitions route outside the box. Composite children resolve after parsing, local sentinels stay local, and positioned notes retain their side and bounds. Composite transition labels have dedicated clearance. Cross-boundary child transitions explicitly reject instead of silently disappearing. |
| MER-184–217 | ER cardinalities preserve endpoint meaning; all 4×4 endpoint combinations parse, dotted relationships stay dotted, and spacing leaves a visible segment between glyphs. |
| MER-222 and unsupported forms | Family dispatch matches exact tokens. Known unsupported shorthand/directives, sequence forms and class forms report actionable diagnostics; see the support boundary. |
| Supplemental public contracts | All public DiagramStyle string attributes are XML-escaped at the SVG boundary. Invalid Unicode hex-color inputs safely use the existing fallback instead of panicking. |

Sequence semantics were checked against the upstream Mermaid language: bare
`->`/`-->` are markerless; `->>`/`-->>` use filled heads, `-)`/`--)` async
open heads, and `-x`/`--x` cross markers. Initial audit assertions that treated
closed reply arrows as forward requests were corrected and rerun; production
behavior was not changed to satisfy those mistaken assertions.

### CLI, math, fonts and distribution fixes

| Stories | Correction and observable result |
| --- | --- |
| CLI-031–038/095–099 | Public width validation is shared with the CLI. Standalone padding rejects negative/nonfinite values and finite canvas overflow, while accepting zero/fractional padding. |
| CLI-126/127/136/207 | All six consumed Alacritty palette fields validate six-digit hex colors; `0x` normalizes to `#`. Both parser diagnostics survive syntax fallback, and palette attributes cannot break XML. |
| CLI-140/141/144 | Math preserves Unicode, text spaces/digits and independent source sub/superscript arguments, without flattening intentionally nested scripts. |
| CLI-157/158/160–162 | Math retains aligned columns, cases/inequalities, nested matrix cells and stretchy delimiters. Converter token contents are escaped before MathML parsing. |
| CLI-166/167/193 | Unsupported math commands return an error so Markdown can warn and preserve source fallback, instead of painting converter-error text as a successful formula. |
| CLI-174/175/201 | Measurement loads valid cwd fonts and reports missing-font errors. PNG/PDF sans and mono fallback follow the actual CosmicText measurement families rather than font-name guesses. The host-font reproduction measured Noto Sans Bold but exported condensed Liberation Sans Narrow (document advance 179.06px versus 137.91px); the corrected PNG/PDF advances agree with measurement. Local-only generic/explicit references and host raster ink-width checks distinguish the wrong family; actual PNG/PDF spacing was inspected. |
| CLI-186/191 | Release configuration packages both binaries under the existing archive name. The installer optionally installs the standalone sibling and still accepts older single-binary archives. README environment overrides apply to the downstream shell. No release or real installation was performed. |
| CLI-208/209 | Direct/Theme-derived math colors are XML-escaped while named/rgb values remain valid. Backslash tokens are consumed atomically so escaped underscore remains literal and cannot leak a temporary script slot. |

### Trustworthy reruns

The audit runners were also reviewed independently. CLI probes fingerprint both
their source and linked library; repeated runs use unique output directories,
remove expected outputs before execution, require fresh outputs, and link the
actual new result alongside immutable initial evidence. A successful command
that produces no new file cannot reuse an old SVG to pass. Timeout evidence
retains partial output.

Markdown and Mermaid build current Cargo artifacts before probing. MER-223 now
executes 15 actual SVG/PNG/PDF exports across all five families; an ignored
`exports` check that could previously pass without PNG/PDF was corrected, and
unknown check keys fail explicitly. Generated inputs and binaries remain in
ignored scratch; only reusable runner sources/fixtures are committed.

Imports update only executed stories. No cached snapshot is bulk-imported as a
new retest. Earlier targeted CLI retests were invalidated with pending events;
their history remains intact. Final verification requires a new executed rerun
after all production owners freeze. Geometry checks using the same text-measure
API as the renderer establish consistency, not independent visual fit; native
PNG/PDF inspection remains necessary.

### Final verification

The replacement coordinated rerun executed all 569 stories against a frozen
source and runner manifest. Current canonical results are:

| Workstream | Executed | Passed | Unsupported | Failed / blocked |
| --- | ---: | ---: | ---: | ---: |
| Markdown | 137 | 123 | 14 | 0 |
| Mermaid | 223 | 199 | 24 | 0 |
| CLI, exports, themes, math, fonts, installation | 209 | 209 | 0 | 0 |
| **Total** | **569** | **531** | **38** | **0** |

The 38 unsupported stories remain unverified in the dashboard denominator;
531/569 is 93.3%, not a claim of full language compatibility. Supported stories
all passed their executed contracts. Baseline errors, assertion corrections,
targeted checks and the invalidated first coordinated run remain in history.
The final run followed the last branch-label and measurement/export font fixes;
earlier passes were not replayed as final evidence.

Reproduction commands are in [feature-audit/README.md](feature-audit/README.md).
On the live dashboard, the final source manifest and exact per-command logs are
available under these artifact batches:

- Markdown: `markdown/retest/20261018T232714232666Z`.
- Mermaid: `mermaid/20261018T232712208434Z` and
  `mermaid/visual-20261018T232712208434Z`.
- CLI: `cli/coordinated-final-v2`.
- Repository checks and baseline review: `cli/combined-final-v2`.

Each owner archived the replacement manifest and commit, checked source hashes
before and after execution, and published only newly executed results. Mermaid
also checked after publishing; its final stamp occurs once per row. MER-223
executed all 15 five-family SVG/PNG/PDF exports. Historical errors, exact
reproduction commands, output, fixes and retests remain in the canonical CSV.

Markdown exercised 397 CLI/public-renderer variants, producing 394 SVG/PNG
outputs and three expected width rejections. Its reviewer inspected all 20 PNG
contact sheets and both sheets covering 18 fresh PDFs at light, narrow-light and
narrow-dark settings. Mermaid reviewed fresh PNG/PDF examples from all five
families plus the unchanged asymmetric showcase. CLI reviewed fresh host and
isolated-local-font PNG/PDF output, including Unicode, scripts and matrices.

Repository verification passed `cargo fmt --check`, all 142 unit tests,
`cargo clippy --all-targets -- -D warnings`, fresh binary/library builds and the
existing quality smoke renderer/checker. The coordinator inspected smoke and
five-theme demo comparisons before accepting exactly 13 existing baseline
assets (11 demos and two smoke SVGs). Both smoke and demo sync checks then passed.
Original assets, normalized diffs, environment inventories and acceptance logs
remain in `cli/combined-final-v2`. Delimiters, markers, quote clearance and
diagram semantics explain the changes; no baseline was copied just to hide a
failing comparison.

Baseline regeneration used the existing scripts with verified DejaVu-only
system Fontconfig inventories in CosmicText and both export engines, retaining
repository-local font inputs. This host ran Ubuntu 24.04.5 and Rust 1.98.1;
CI pins Rust 1.97.1, so local checks are not an execution of that exact CI image.
The independent final ledger check matched all 569 IDs to source definitions,
matched every latest event to fresh execution evidence, verified all earlier
history strings remained intact, and confirmed the public API matches the CSV.

## Showcase and remaining limitations

The showcase uses real native output from `tests/fixtures/showcase*` inputs.
The seven selected post-freeze workflows cover a mixed document, math,
branch/cycle flowchart, sequence, ER model, theme change and PDF export.
Captions show reproducible commands; this is a silent captioned output tour,
not a recording of typing or an interactive product UI. The generator rebuilds
current binaries, rerenders each command and records its output before encoding
seven individual ten-second clips and an 80-second 1920×1080/24fps H.264 tour.
Diagrams retain their whole context; documents pan at a readable scale.

All clips and the master decoded without errors. Eleven extracted master frames
were inspected, including each scene and both title cards. Chromium playback
reached 80 seconds/ended with no media error and 1920 decoded frames (three
display frames dropped on this host). Video metadata, command logs, the encoded
frame sheet and browser observations remain in the showcase artifact directory.
Selection changed only showcase metadata and timestamps; test histories were
not replayed or modified.

[Watch the combined tour](https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/artifacts/feature-audit/showcase/markie-product-tour.mp4).
Individual ten-second clips:

| Workflow | Verified story | Clip |
| --- | --- | --- |
| Mixed document | MD-031 | [Watch](https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/artifacts/feature-audit/showcase/01-MD-031.mp4) |
| Math in prose | CLI-144 | [Watch](https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/artifacts/feature-audit/showcase/02-CLI-144.mp4) |
| Decision and feedback loop | MER-095 | [Watch](https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/artifacts/feature-audit/showcase/03-MER-095.mp4) |
| Sequence workflow | MER-128 | [Watch](https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/artifacts/feature-audit/showcase/04-MER-128.mp4) |
| Entity relationships | MER-189 | [Watch](https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/artifacts/feature-audit/showcase/05-MER-189.mp4) |
| Theme change | CLI-112 | [Watch](https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/artifacts/feature-audit/showcase/06-CLI-112.mp4) |
| PDF review packet | CLI-051 | [Watch](https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/artifacts/feature-audit/showcase/07-CLI-051.mp4) |

Remaining limits are deliberate and visible:

- This inventory covers the implemented feature surface and adversarial cases;
  it is not proof over every Markdown, Mermaid or LaTeX input.
- Mermaid is a supported subset. Arbitrary graph collision avoidance, full
  language compatibility, browser HTML/CSS and complete TeX are outside scope.
- Unsupported image/syntax stories stay in the denominator and cannot become
  verified successes merely because rejection/fallback works correctly.
- Fonts affect metrics and glyph coverage. Real usable fonts are required;
  repository zero-byte font placeholders are not a font bundle.
- Existing muted code/fallback colors and sequence palettes can be hard to read,
  especially Dracula and Solarized Dark. The audit corrected the confirmed
  flowchart pill contrast regression; it does not certify theme accessibility.
- PDF text is exported as paths, so it is not searchable/selectable text.
- Release/installer checks use static configuration and disposable intercepted
  download/install sandboxes. Other OS/architecture releases were not built or
  published; no remote assets or installed production binaries were replaced.
- Ignored evidence/media remain on this runner; the public dashboard and media
  URLs require the existing server and tunnel to keep running. Reproduction
  instructions are in [feature-audit/README.md](feature-audit/README.md).

Delivery is local: production corrections, audit sources, reviewed baselines,
fixtures and documentation are committed under the requested lsj5031 identity.
No push, merge, release or production installation was performed. The dashboard,
CSV download, completed report and media remain available through the existing
read-only service while its server and tunnel are running.

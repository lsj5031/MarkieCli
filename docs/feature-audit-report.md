# MarkieCli feature audit

Status: in progress. The canonical story inventory and live status are in
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
| Markdown | 96 | 32 | 9 |
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

Fixes and the full post-fix rerun are in progress. Historical errors, exact
reproduction commands, actual output, fixes, and retest results remain in the
canonical spreadsheet and linked per-story evidence. No initial pass is counted
as a verified post-fix pass.

## Showcase and remaining limitations

Pending post-fix validation. The final report will link the individual feature
clips, the combined product tour, and any unresolved environmental or platform
limitations. A blocked or unsupported story will not be reported as a passing
test.

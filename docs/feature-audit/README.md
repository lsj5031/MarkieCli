# Live feature audit

`docs/feature-audit.csv` is the only durable audit state. The dashboard reads it
every 2.5 seconds; it does not maintain its own database or browser storage.

```sh
python3 scripts/feature-audit.py import .amp/in/audit/inventory.json
python3 scripts/feature-audit.py serve --port 8766
```

Imports accept a JSON array of objects. Every object needs an ID matching
`MD-001`, `MER-001`, or `CLI-001` (three or more digits). Values are strings.
Only supplied columns are merged. New stories default to `inventoried`.
Valid statuses: `inventoried`, `testing`, `failed`, `fixing`, `retesting`,
`passed`, `unsupported`, `blocked`. `updated_at` is set automatically in UTC;
the dashboard displays Auckland time.

`errors`, `fix`, and `retest` are retained as timestamped histories. Empty
values cannot erase them. Repeated identical errors/fixes are deduplicated;
each supplied retest is recorded as a new event. Set `retest` to
`passed: <observed result and evidence>` after actually rerunning the test,
or `failed: ...` / `blocked: ...` as appropriate. `status: passed` alone does
not count as verified. Changing status without a fresh retest invalidates
the previous retest result. The dashboard never rounds partial coverage to
100%, and blocked/unsupported stories stay in the denominator. These are
worker-reported results; the tracker does not execute tests itself.

## Reproduce the audit

Run from the repository root on Linux with Python 3, Rust/Cargo (Edition 2024),
a native linker and build libraries, system fonts, Fontconfig (`fc-match`),
`tar`, `sh`, `bash`, and Poppler (`pdfinfo`, `pdftoppm`). Install normal system
build dependencies on a fresh host; the ignored extracted toolchain below is
specific to the current audit runner and is not supplied by Git:

```sh
export PATH="/home/leo/.cargo/bin:$PWD/.amp/in/toolchain/root/usr/bin:$PATH"
export LD_LIBRARY_PATH="$PWD/.amp/in/toolchain/root/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
```

On a fresh checkout, first generate the ignored case snapshots from the tracked
definitions. These commands do not import results into the canonical CSV:

```sh
python3 -B tests/audit/markdown/run.py --inventory
python3 -B tests/audit/cli/inventory.py
python3 -B tests/audit/mermaid/run.py inventory
cargo build --bins --lib
```

Snapshots live at `.amp/in/audit/{markdown,cli,mermaid}-inventory.json`;
Mermaid also writes `.amp/in/artifacts/feature-audit/mermaid/cases.json`.
Do not import cached inventory/result JSON as fresh observations. MD and CLI
inventory regeneration can retain old local observations; Mermaid inventory
regeneration resets its local snapshot. Neither changes canonical history.
CLI expects the freshly built binaries and library in `target/debug`; if using
a different Cargo target directory, set `MARKIE_BIN_DIR` to its `debug` directory
(including both binaries, `libmarkie.rlib`, and `deps/`).

After production sources are frozen and the final rerun is authorized, execute
all stories without case filters:

```sh
python3 -B tests/audit/markdown/run.py --phase retest
python3 -B tests/audit/cli/run.py --retest
python3 -B tests/audit/mermaid/run.py test --phase full
```

These execute tests and publish new per-story events, not cached results.
MD and MER also build current code themselves. For targeted development checks,
use MD `--phase targeted MD-001`, CLI `--case CLI-001` without `--retest`, or
MER `test --phase targeted --case MER-001`. MD targeted mode does not publish;
CLI ordinary runs publish baseline statuses/evidence without passed retest
events (`--no-import` keeps them local); MER targeted runs publish explicitly
targeted results. None substitutes for the full post-freeze rerun. Unsupported
and blocked stories remain unverified; a targeted success must not be replayed
as a final passed event.

Visual review additionally needs Pillow in the caller's Python environment and
Poppler for PDF rasterization. This host has Pillow and `imageio-ffmpeg` in
`.amp/in/media-tools`; use `.amp/in/media-tools/bin/python` for image/video
helpers. A fresh host can create an equivalent environment with
`python3 -m venv .amp/in/media-tools` followed by
`.amp/in/media-tools/bin/python -m pip install Pillow imageio-ffmpeg`.
Keep the same fonts when comparing renders: this host resolves sans to Noto
Sans and monospace to DejaVu Sans Mono; the MD preview helper explicitly needs
`/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`. Inspect rendered outputs and
PDF previews rather than treating structural assertions as visual proof.
The helpers are `tests/audit/markdown/preview.py ARTIFACT_BATCH_DIRECTORY`,
`tests/audit/cli/visuals.py fixed`, and
`tests/audit/mermaid/render_evidence.py --run ARTIFACT_BATCH_DIRECTORY`.

```json
[
  {
    "id": "MD-001",
    "area": "Markdown",
    "feature": "Headings",
    "user_story": "As a reader I can distinguish heading levels.",
    "expected_behavior": "H1–H6 retain hierarchy in the rendered output.",
    "owner": "markdown",
    "status": "inventoried"
  }
]
```

This is an import example, not a completed test. Error/fix/retest updates can
contain just the ID and changed fields. Writers serialize with `flock` on
`.amp/in/audit/import.lock` and replace the CSV atomically. Do not directly
rewrite the CSV while workers are importing.

Evidence accepts newline- or semicolon-separated repository-relative or absolute
paths under `.amp/in/audit/` or exactly
`.amp/in/artifacts/feature-audit/{markdown,mermaid,cli,showcase}/`. The matching browser
routes are `/artifacts/<audit-relative-path>` and
`/artifacts/feature-audit/<area>/<path>`. Supported evidence types are PNG,
JPEG, WebP, SVG, PDF, text/log/JSON/Markdown/CSV in the audit and owner roots.
The showcase root allows only PNG and MP4; MP4 is served only there, streamed
with `video/mp4` and an accurate `Content-Length`. Range requests receive the
complete file with status 200. SVG and PDF
download as attachments. HTML, scripts, hidden files, directory listings and
paths escaping the allowed roots are not served. A nonempty `showcase` value
selects a story unless it is `no`, `false`, `none` or `0`.

The lifecycle panel separates inventory, baseline testing, fixes, full retest,
showcase, and report. All three owners handed off the current code-derived
inventory; this is not proof of every possible input. Baseline counts require a
command, evidence, and a result-bearing status. Fix counts use retained fix
history. Retest counts use the latest separate passed retest and passed status.
Showcase availability requires both the exact
`.amp/in/artifacts/feature-audit/showcase/markie-product-tour.mp4` file and at
least one CSV-selected story with a verified passed retest. Draft covers and
intro clips do not count. The report is draft until its own `Status:` line says `complete`
or `completed`. No row status marks the entire audit complete.

Read-only routes: `/`, `/style.css`, `/app.js`, `/api/audit`,
`/download/feature-audit.csv`, `/download/feature-audit-report.md`, and the evidence routes above. The server binds
`0.0.0.0`; it provides no write endpoints. Use existing private network access.
On this runner, `http://192.168.1.164:8766` works only if the client can reach
that LAN. If the runner is accessible over SSH as `Leo-Claw`, run locally:

```sh
ssh -N -L 8766:127.0.0.1:8766 leo@Leo-Claw
```

Then open `http://localhost:8766` on the client machine. No Tailscale or public
tunnel is configured by this tool.

For this audit, a separately managed OpenTunnel route exposes the read-only
dashboard at <https://1cd7530c4b998844.zqk6knxethul.opentunnel.xyz/>. Its local
target is `127.0.0.1:8766` and its profile is `markie-audit`. The audit server
and the tunnel service must both remain running for the public URL to work.

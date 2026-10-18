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

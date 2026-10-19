# Markdown audit runner

Run from the repository root with Cargo, rustc, a native linker and required
shared libraries available in the caller's environment. The runner inherits
that environment and builds the current CLI and public library before testing;
it obtains artifact paths from Cargo's JSON output.

On the audit's original Linux runner, the local toolchain workaround is:

```sh
export PATH="/home/leo/.cargo/bin:$PWD/.amp/in/toolchain/root/usr/bin:$PATH"
export LD_LIBRARY_PATH="$PWD/.amp/in/toolchain/root/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
```

```sh
python3 tests/audit/markdown/run.py MD-014 MD-115
python3 tests/audit/markdown/run.py --phase retest
```

The default `targeted` phase saves isolated evidence without publishing final
passes. `retest` executes all stories, or supplied IDs, and imports only those
stories' new events into the feature dashboard. `baseline` is for the initial
capture; the completed audit's frozen baseline must remain intact.
Imports contain only new per-story execution events; old error/fix histories
are not replayed and the runner never publishes showcase selections.

Compatibility regressions extend the existing MD-052 and MD-065 through MD-068
stories with positive alert-kind and formatted-cell contracts. Run them without
publishing final passes while a coordinated source freeze is pending:

```sh
python3 -B tests/audit/markdown/run.py --phase targeted MD-052 MD-065 MD-066 MD-067 MD-068
```

These cases check per-text styles and decorations, scoped resets, explicit blank
breaks, variable row heights, literal code payloads and padded boxes, complete
graphemes, and measured bounds of text/boxes/decorations within their cells.
Only executed stories appear in each batch's results/counts; unknown IDs fail.

Case definitions and the public-library probe live here, along with a reusable
16×8 JPEG fixture. Generated Markdown/image inputs and the relative symlink
sandbox live under ignored `.amp/in/audit/markdown-inputs`. Commands, diagnostics,
SVGs and checks live under `.amp/in/artifacts/feature-audit/markdown`; targeted
and retest batches receive unique timestamped directories and retain an `input.md`
copy per executed story. Remote-image cases
use a temporary loopback HTTP server, including bounded timeout/size failures.

Optional PNG previews and contact sheets require Pillow in the caller's Python
environment:

```sh
python3 tests/audit/markdown/preview.py .amp/in/artifacts/feature-audit/markdown/targeted/BATCH
```

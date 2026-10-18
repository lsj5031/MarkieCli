# CLI feature audit

Run from the repository root with Python 3, Rust/Cargo, system fonts,
Fontconfig (`fc-match`), `tar`, `sh`, `bash`, `pdfinfo`, and `pdftoppm` on PATH:

```sh
cargo build --bins --lib --offline
python3 -B tests/audit/cli/inventory.py
python3 -B tests/audit/cli/run.py --case CLI-208
python3 -B tests/audit/cli/visuals.py fixed
```

The runner discovers the repository from its own location and inherits the
caller's PATH and LD_LIBRARY_PATH. `MARKIE_BIN_DIR` overrides `target/debug`;
that directory must contain both binaries, `libmarkie.rlib`, and `deps/`.
On the audit host only, locally extracted build dependencies needed this
environment prefix (it is not required on normally provisioned systems):

```sh
PATH="$HOME/.cargo/bin:$PWD/.amp/in/toolchain/root/usr/bin:$PATH" \
LD_LIBRARY_PATH="$PWD/.amp/in/toolchain/root/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
cargo build --bins --lib --offline
```

Inventory regeneration preserves prior observations. Every repeated story
uses a fresh output directory; the public probe is rebuilt when its source or
linked library changes. Generated fixtures, binaries, and logs live under
ignored `.amp/in/`. Initial results remain immutable. Ordinary runs import
only affected statuses/evidence and never append post-fix retest events;
`--no-import` retains observations locally. The explicit `--retest` option is
reserved for the coordinator's authorized source-freeze rerun.

CLI/probe execution has a 60-second timeout and 1 GiB address-space limit;
probe compilation has a 4 GiB limit. Install/download checks intercept network
commands and install disposable fixture executables only inside each sandbox.
They do not execute releases or verify actual remote assets/platforms.

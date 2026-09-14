---
kind: bug
status: mitigated
tags:
- windows
- vdi
- toolchain
- build
- cluster/repro-env-diverges-from-gate-env
closed: 2026-08-31
no_fix_commit: Not applicable, and the bug file's own Fix section says so — a missing third-party install on the VDI (MSVC Build Tools, with link.exe shadowed by Git coreutils), not a codescout code path. No code change exists and none can. The mitigation is a GNU-hosted toolchain invocation, documented in the Workarounds section, and it deliberately does not cover the local-embed feature. Declared 2026-09-11 during a doctor terminal_status_without_fix_anchor sweep.
opened: 2026-08-31
owner: marius
related: []
severity: medium
unverified: root cause is inferred from the absence of any VC++ install (vswhere) — not confirmed by IT/asset records for this VDI; no fix applied, only a workaround
---

# BUG: This VDI has no MSVC C++ Build Tools installed — `cargo build`/`test`/`clippy` against the default `x86_64-pc-windows-msvc` target fail with a garbled linker error instead of a clear "linker not found"

## Summary

On this VDI (`MAILINCA.BRN.002`), running the documented pre-commit gate
(`cargo clippy --workspace --all-targets --features local-embed -- -D warnings`)
against the default host target fails during linking. The failure does not read as
"no linker" — it reads as a malformed linker invocation, because `link.exe` on PATH
resolves to Git for Windows' own coreutils `link` (a hard-link utility), not MSVC's
linker. The real MSVC toolchain is not installed anywhere on the machine.

## Symptom (Effect)

```
error: linking with `link.exe` failed: exit code: 1
  |
  = note: "link.exe" "/NOLOGO" "...\\build_script_build....rcgu.o" ...
  = note: some arguments are omitted. use `--verbose` to show all linker arguments
  = note: link: extra operand 'C:\...build_script_build.....rcgu.o'
          Try 'link --help' for more information.

error: could not compile `serde_core` (build script) due to 1 previous error
error: could not compile `quote` (build script) due to 1 previous error
error: could not compile `num-traits` (build script) due to 1 previous error
error: could not compile `parking_lot_core` (build script) due to 1 previous error
```

`link: extra operand '...'. Try 'link --help' for more information.` is GNU
coreutils' error format, not MSVC `link.exe`'s (which would say `LINK : fatal
error LNK....`). That is the tell that the wrong binary is being invoked.

## Reproduction

```
git rev-parse HEAD   # aa242912 (branch: experiments)
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
```

Fails immediately on the first build-script link. Also reproduces with plain
`cargo build` / `cargo test --workspace` against the default target.

## Environment

- Windows 11 Enterprise 10.0.26200, corporate VDI (`MAILINCA.BRN.002`)
- `rustup show`: default host `x86_64-pc-windows-msvc`, active toolchain
  `1.97.1-x86_64-pc-windows-msvc` (pinned by `rust-toolchain.toml`, which does not
  override the host — it only adds `x86_64-pc-windows-gnu` as an extra installed
  target for CI's cross job)
- `which -a link` → `/usr/bin/link` (Git for Windows' `usr/bin/link.exe`, coreutils
  8.32); no other `link.exe` found anywhere under `Program Files` / `Program Files
  (x86)` on the whole `C:` drive
- `vswhere -all -products '*'` lists exactly one product: SQL Server Management
  Studio 22. No Visual Studio / Build Tools / VC++ workload installed at all.
- `Get-Command link.exe` from native PowerShell also finds nothing; no VS/MSVC/
  BuildTools entry anywhere in `$env:Path`.
- GNU/MinGW toolchain IS present: `scoop`-installed `gcc` at
  `/c/Users/MAILINCA.BRN.002/scoop/apps/mingw/current/bin/gcc`, and rustup has both
  `stable-x86_64-pc-windows-gnu` and `x86_64-pc-windows-gnu` (target) installed.
- `target/release/codescout.exe` is dated 2026-08-19; a separate, untracked
  `target-gnu-rebuild/release/codescout.exe` (built with `--target
  x86_64-pc-windows-gnu`, its own `CARGO_TARGET_DIR`) is dated 2026-08-21 — i.e.
  the msvc-target binary predates the gnu workaround by ~2 days, consistent with
  the MSVC toolchain having been removed from this VDI sometime in that window.

## Root cause

No MSVC C++ Build Tools (`cl.exe`/`link.exe`/`lib.exe` from a VC++ workload) are
installed on this VDI. When rustc's `x86_64-pc-windows-msvc` codegen looks up
`link.exe` on `PATH`, the only binary that name resolves to is Git for Windows'
bundled coreutils `link` (a hard-link utility living at
`.../Git/usr/bin/link.exe`), which shares a PATH-visible name with the real linker
but accepts a completely different command line — hence the "extra operand"
coreutils error instead of an MSVC linker diagnostic or a "command not found".

*Measured 2026-08-31: `which -a link` → single match, Git's coreutils link;
`vswhere -all -products '*'` → one product (SSMS), no VC++ install; native
PowerShell `Get-Command link.exe -All` → no matches; no `Program Files*` entry
under `Microsoft Visual Studio` beyond the bare Installer/Shared scaffolding.*

This is a VDI environment gap, not a codescout code defect — filed per CLAUDE.md's
"open a bug file for ANY... tool quirk noticed during work" (this one repeatedly
misdirects debugging toward "clippy/librarian regression" because the failure
signature is a cascading build-script link error, not an obviously-environmental
one).

## Evidence

### vswhere output (full)
```
[ { "installationName": "SSMS/22.7.2+11919.86", "productPath": "...SSMS.exe", ... } ]
```
Single entry, no VS/BuildTools product.

### PATH probe (native PowerShell)
```
Get-Command link.exe -All -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
# (no output)
$env:Path -split ';' | Select-String -Pattern 'Visual Studio|MSVC|BuildTools'
# (no output)
```

## Hypotheses tried

1. **Hypothesis:** `link.exe` is present but shadowed later in PATH by Git's
   coreutils version (a PATH-ordering fix would suffice).
   **Test:** `find` over all of `C:\Program Files*` for `link.exe`; `vswhere`
   full product listing.
   **Verdict:** rejected — no other `link.exe` exists on the machine at all. This
   is not a PATH-ordering problem, it's a missing-install problem.

2. **Hypothesis:** MSVC tooling exists under a nonstandard path (e.g.
   `C:\BuildTools`, a portable install).
   **Test:** `ls -d /c/BuildTools`, broad `find` for `*visual*` / `*build*tools*`
   under `C:\`.
   **Verdict:** rejected — not found.

## Fix

Not applicable — this is a missing third-party install on the VDI, not a
codescout code path. No code change.

## Tests added

N/A — environment gap, not a code defect. Nothing in the test suite currently
detects "the msvc linker on PATH is actually coreutils `link`"; that could be a
worthwhile `scripts/` sanity check for onboarding new Windows machines, but is
out of scope for this bug file.

## Workarounds

**Partial workaround only — does not cover the `local-embed` feature.** Build/test
against the GNU *toolchain host* (not just `--target`: cargo always compiles
build scripts/proc-macros for the host triple, so `--target
x86_64-pc-windows-gnu` alone still needs a working `link.exe` — the whole
toolchain has to be GNU-hosted):

```
RUSTUP_TOOLCHAIN=1.97.1-x86_64-pc-windows-gnu CARGO_TARGET_DIR=target-gnu-rebuild cargo clippy --workspace --all-targets -- -D warnings   # WITHOUT --features local-embed
RUSTUP_TOOLCHAIN=1.97.1-x86_64-pc-windows-gnu CARGO_TARGET_DIR=target-gnu-rebuild cargo test --workspace --no-default-features
RUSTUP_TOOLCHAIN=1.97.1-x86_64-pc-windows-gnu CARGO_TARGET_DIR=target-gnu-rebuild cargo test --workspace
```

**`--features local-embed` cannot be satisfied on this VDI at all right now**,
under either toolchain:

- msvc host: blocked by this bug (no linker).
- gnu host: blocked separately — `ort-sys` (the ONNX Runtime bindings
  `local-embed` pulls in) ships no prebuilt binary for
  `x86_64-pc-windows-gnu`, only for msvc. Measured 2026-08-31:
  `RUSTUP_TOOLCHAIN=1.97.1-x86_64-pc-windows-gnu cargo clippy --features
  local-embed ...` fails with `ort-sys@2.0.0-rc.11: ort does not provide
  prebuilt binaries for the target x86_64-pc-windows-gnu`.

Confirmed independently: the codescout MCP server instance actually running on
this VDI right now (serving these very tool calls) reports
`embedding_compiled_in: ["remote"]` — no local backend — and `index(action="build")`
fails with `Local embedding requires the 'local-embed' feature`. So the gap is
real at runtime, not just theoretical.

CLAUDE.md's clippy gate command specifically requires `--features local-embed`
("the long clippy form is the gate, not garnish" — it's what reaches
`codescout-embed`'s feature-gated `local` module). **On this VDI, no toolchain
currently reaches that code path at all.** Treat any GNU-toolchain gate run here
as covering everything *except* the `local-embed`-gated code, not as a full
substitute for the documented gate.

Durable fix: install "Build Tools for Visual Studio" (C++ build tools workload)
on this VDI so the default `x86_64-pc-windows-msvc` target works again — this is
the only path that can build `local-embed` at all here.
## Resume

If VC++ Build Tools get reinstalled on this VDI, re-verify with
`vswhere -all -products '*'` and `which -a link` (should show the real
`link.exe` ahead of Git's), then re-run the documented four-command gate
against the default (msvc) target and archive this file with that as the
closing evidence. Until then, use the GNU-target workaround above for local
gate runs on this machine specifically — this is a machine-local condition,
not a repo-wide one.

## References

- `target-gnu-rebuild/` (untracked, this VDI only) — the existing GNU-target
  workaround build this bug file documents.
- CLAUDE.md § *Development Commands* — the four-command gate this bug's
  workaround substitutes a target for.
- `.github/workflows/ci.yml:260` — CI's windows-gnu cross job (Linux + wine),
  not the same runtime as this workaround.

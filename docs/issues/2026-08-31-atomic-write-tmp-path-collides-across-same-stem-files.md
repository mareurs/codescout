---
id: '531f24e980262065'
kind: bug
status: open
title: 'BUG: atomic_write''s tmp path REPLACES the extension, so same-stem files share one tmp path — concurrent writes corrupt across files'
tags:
- atomic-write
- concurrency
- shared-checkout
- data-loss
- latent
closed: ''
opened: 2026-08-31
owner: marius
related:
- docs/issues/archive/2026-08-28-atomic-write-leaks-its-temp-file-on-failure.md
severity: high
unverified: NO OBSERVED INSTANCE. The mechanism is measured (with_extension verified by running it; 7 collision groups enumerated from git ls-files) but no corruption has been seen in the wild, and no test yet demonstrates the interleaving. Severity high is a judgement about the consequence, not an observation of frequency.
---

# BUG: `atomic_write`'s tmp path REPLACES the extension, so same-stem files share one tmp path — concurrent writes corrupt across files

## Summary

`src/util/fs.rs:63` derives the staging path with `path.with_extension("tmp")`, which
**replaces** the extension rather than appending to it. So `Cargo.toml` and `Cargo.lock` both
stage through `Cargo.tmp`. Two concurrent `atomic_write`s to same-stem files in one directory
race on a single staging file, and the loser does not merely lose its own write — it renames
the *other* file's content onto its own target. **Cross-file corruption, not
last-writer-wins.**

Latent: no instance has been observed. The mechanism is measured, the reachability is
enumerated, and the consequence is a silently corrupted tracked file.

## Symptom (Effect)

None observed — this was found by reading `atomic_write` during a verify-open sweep of a
neighbouring bug, and confirmed by probe rather than by a failure.

The predicted symptom is the reason for filing: a tracked file whose content is *another
file's* content, with no error raised anywhere, no partial write, and a successful exit from
both writers. Nothing in the failure surfaces; both callers are told the write succeeded.

## Reproduction

### The mechanism, verified by running it (not read from the docs)

Compiled and executed standalone, outside the project tree:

```rust
use std::path::Path;
fn main() {
    for p in ["docs/x.md", "docs/x.rs", "docs/x", "docs/x.tar.gz", "docs/.gitignore"] {
        println!("{:<18} -> {}", p, Path::new(p).with_extension("tmp").display());
    }
}
```

```
docs/x.md          -> docs/x.tmp
docs/x.rs          -> docs/x.tmp        <- collides
docs/x              -> docs/x.tmp        <- extensionless collides too
docs/x.tar.gz      -> docs/x.tar.tmp    <- only the LAST extension is replaced
docs/.gitignore    -> docs/.gitignore.tmp   <- dotfile is a stem, so this APPENDS; no collision
```

Three of five inputs collapse onto one path. The extensionless case was not anticipated and
widens the class; the dotfile case is safe, which is worth knowing so nobody "fixes" it twice.

### Reachability in this repo — 7 collision groups, from `git ls-files`

| staging path | real files that share it |
|---|---|
| `Cargo.tmp` | `Cargo.toml`, `Cargo.lock` |
| `.env.tmp` | `.env.amd`, `.env.cpu`, `.env.example`, `.env.gpu`, `.env.lite` — **five** |
| `src/prompts/source.tmp` | `source.md`, `source.rs` |
| `src/dashboard/static/dashboard.tmp` | `dashboard.css`, `dashboard.js` |
| `scripts/run-tc-benchmark.tmp` | `.py`, `.sh` |
| `tests/fixtures/nav-eval-rust/Cargo.tmp` | `Cargo.toml`, `Cargo.lock` |
| `tests/fixtures/rust-library/Cargo.tmp` | `Cargo.toml`, `Cargo.lock` |

`src/prompts/source.md` / `source.rs` is the pair that should decide the severity.
`CLAUDE.md` § *Prompt Surface Consistency* makes `source.md` a load-bearing surface with three
consumers that must stay consistent, and `source.rs` is its module — both are edited by
sessions, and `edit_markdown` and `edit_code` both route through `atomic_write`.

### The interleaving

*`codescout-ae`'s trace, and it is sharper than the clobber reading I first had:*

1. Session **A** calls `atomic_write("x.md", …)` → writes x.md's content to `x.tmp`.
2. Session **B** calls `atomic_write("x.rs", …)` → overwrites `x.tmp` with x.rs's content.
3. Session **A** renames `x.tmp` → **`x.md`**.

`x.md` now holds `x.rs`'s content. B's rename then either fails (no `x.tmp`) or moves whatever
is there. **The corruption crosses files**, which is a different severity class from a lost
update: a lost update leaves valid content, this leaves the wrong file's bytes under a tracked
name.

## Environment

Any platform — `Path::with_extension` semantics are not OS-specific. The race needs two
concurrent writers, which this checkout has by construction: measured 2026-08-31, **five**
Claude sessions with cwd here, and `atomic_write` has **15 call sites across 9 files**
(`references(atomic_write)`), including `edit_file`, `edit_markdown`, `edit_code`,
`memory`, `operator_rules`, `library/registry` and `symbol/edit`.

## Root cause

`src/util/fs.rs:63`:

```rust
let tmp = path.with_extension("tmp");
```

`Path::with_extension` replaces any existing extension. The staging path is therefore a
function of the **stem and directory only**, discarding the one component that distinguishes
`x.md` from `x.rs`. Two facts have to hold for the bug, and both do: the tmp path is not
unique per target, and the rename is unconditional.

Measured 2026-08-31 by the probe above. Note this is **not** the same defect as
`docs/issues/archive/2026-08-28-atomic-write-leaks-its-temp-file-on-failure.md`, which is about the
initial write's failure path in the same function; that one is being fixed separately and its
fix does not touch the path derivation.

## Fix

Make the staging path a function of the whole filename, not the stem. The minimal form:

```rust
let mut name = path.file_name().unwrap_or_default().to_os_string();
name.push(".tmp");
let tmp = path.with_file_name(name);
```

`Cargo.toml` → `Cargo.toml.tmp`, `Cargo.lock` → `Cargo.lock.tmp`. Distinct, and it fixes the
extensionless case at the same time.

**This does not make the write race-free**, and the fix should not claim it does. Two writers
to the *same* target still race, and that is inherent to write-then-rename without a lock —
what this removes is the *cross-file* case, where the corruption lands somewhere the caller
never named. A per-target unique suffix (pid, or a counter) would go further and is a separate
decision.

### Cost of the change — checked, not assumed

Everything that currently depends on the tmp-path shape (`grep` over `*.rs`/`*.toml`/`*.sh`/`*.yml`):

| site | depends how | survives the change? |
|---|---|---|
| `src/util/fs.rs:63` | the derivation itself | this is the change |
| `src/util/fs.rs:469` | a test computing the expected tmp path via `with_extension` | **needs updating** — belongs to the leak fix now in flight |
| `src/tools/guide_ledger.rs:639` | `.filter(\|n\| n.ends_with(".tmp"))` | **yes** — `x.md.tmp` still ends with `.tmp` |

So one test to update and nothing else. The `ends_with` predicate is the shape that makes this
cheap, and it is worth noting that a `with_extension`-based cleanup anywhere would not have
been.

## Tests added

None yet. The regression test should assert **distinctness** rather than a literal name —
`tmp_path("x.md") != tmp_path("x.rs")` — because a literal assertion pins the format and would
need rewriting for any future change to the suffix scheme, while distinctness is the actual
contract. A demonstration of the interleaving is harder and probably not worth it; the
distinctness assertion dies on the unfixed function, which is the discrimination that matters.

## Workarounds

None available to a caller — the path derivation is internal and not parameterised. Avoiding
concurrent edits to same-stem files in one directory is the only mitigation, and it is not
enforceable across sessions.

## Resume

Apply the `with_file_name` change at `src/util/fs.rs:63`, update the expected-path computation
at `src/util/fs.rs:469` (coordinate with whoever is finishing the leak fix — that file had 78
uncommitted lines at filing time), and add the distinctness assertion. Confirm it dies on the
unfixed function rather than assuming.

## References

- `docs/issues/archive/2026-08-28-atomic-write-leaks-its-temp-file-on-failure.md` — the other defect in
  the same function, found in the same sweep, being fixed separately. Kept apart deliberately:
  the leak fix is behaviour-preserving on the success path and this is not.
- `docs/trackers/bug-fix-session-log.md` — `W-87`, the sweep that surfaced this by reading the
  code rather than the record.

---
id: c4c14c20936fb3a1
kind: bug
status: fixed
title: 'BUG: a python write through a Path-valued variable is invisible, and the cause it was filed under is not the mechanism'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
topic: provenance correctness
claimed_at: 2026-09-08
claimed_by: c9ab2c8d-dd74-43f4-9940-25756379a312
closed: 2026-09-08
opened: 2026-09-08
severity: medium
---

**Found:** 2026-09-08, while picking up the residual `ad379a7c` handed over after
`docs/issues/archive/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md`.
**Affects:** `scripts/file-provenance.py` — `PY_WRITE_VAR` and the assignment resolver
inside `python_write_targets()`.

## Summary

The python-snippet branch recognises a write only when the path reaches `open(…, "w")` or
`Path(…).write_…` **as a literal, or as a variable assigned a bare string literal**. A path that
is already a `Path` object — the ordinary way anyone writes a multi-line snippet — is not
recognised as a write at all.

`IC-18`: the selector is narrower than the population it names. It is called "python writes"
and it selects "python writes whose target is textually a string at the call site".

## Reproduction

Run against the production `python_write_targets()`, 2026-09-08:

| case | snippet (abbreviated) | targets | verdict |
|---|---|---|---|
| A | `open("src/a.rs","w")` | `['src/a.rs']` | ok |
| B | `Path("src/b.rs").write_text(x)` | `['src/b.rs']` | ok |
| C | `p = "src/c.rs"` … `open(p,"w")` | `['src/c.rs']` | ok |
| D | `p = "src/d.rs"` … `Path(p).write_text(x)` | `['src/d.rs']` | ok |
| **E** | `F = Path("src/e.rs")` … `F.write_text(txt)` | `[]` | **MISS** |
| **F** | `F = pathlib.Path("src/f.rs")` … 3 lines … `F.write_text(q)` | `[]` | **MISS** |
| **G** | `p = Path("src/g.rs")` … `open(p,"w")` | `[]` | **MISS** |
| H | `p = f"src/{name}.rs"` … `Path(p).write_text(x)` | `[]` | correctly missed — not statically resolvable |
| I | `p = "src/i.rs"` … `Path(p).read_text()` | `[]` | ok — reads must not count |
| J | `p = "src/j.rs"` … `open(p).read()` … `open(p,"w")` | `['src/j.rs']` | ok |

## Root cause — two independent narrowings, not one

1. **The write-call selector.** `PY_WRITE_VAR` is
   `Path\(\s*([A-Za-z_]\w*)\s*\)\s*\.write_` — it requires the variable to be *wrapped* in
   `Path(...)` at the call site. A bare `F.write_text(...)`, where `F` is already a `Path`,
   matches no pattern, so cases E and F never reach the resolver at all.
2. **The assignment resolver.** `\bVAR\s*=\s*['"]([^'"]+)['"]` binds only a bare string
   literal, so `p = Path("src/g.rs")` binds nothing — case G.

## One correction to the stated mechanism, which matters for the fix

The residual has twice been described as *"a variable assigned **one line earlier**"* — in the
handover and in conversation. **That is not what the code does.** The resolver is a
`re.search` over the whole `cmd`, so distance is irrelevant: case J resolves across three lines,
and case F fails at three lines for an unrelated reason. A fix aimed at "look back further"
would change nothing and would test green against cases C/D/J, which already pass.

## Fix

**Shipped 2026-09-08 at `4e7a43c0`** (`experiments`), patch-id
`cfd39c0aa876bed590f0fa5c65d0d229501e6214`. Recorded as a pair at fix time: the SHA is
positional and dies when `experiments` is rebased; the patch-id is a content hash of the diff and
survives rebase and cherry-pick. Single-parent, so the patch-id is real — a merge emits no diff
and the pipeline returns empty with exit 0.

**Whole-tree gate green in a single run, in the mandated order:** `cargo fmt --all -- --check`
exit 0 with empty output (the check form deliberately — three peer sessions shared this checkout
and the writing form rewrites their uncommitted Rust), clippy exit 0, LEAN exit 0, DEFAULT exit 0
with **5533 passed / 0 failed** run **last** so `target/debug/codescout` is left librarian-bearing,
and `tests/file-provenance.sh` **110/110**.

**Not verified on Windows or macOS.** Nothing here is platform-specific — the change is four
regexes and a comment — but this ran on Linux only and CI runs on push, which has not happened.

The `IC-18` member and its derivation landed in the same commit; the pre-commit gate refuses the
split, correctly — a class gaining a member without naming it is red in one direction or the other
until both halves land.

**Done 2026-09-08.** Both narrowings widened, plus a third found while fixing them.

### End-to-end validation against known ground truth

`src/peer/server.rs` is the file that returned `UNKNOWN` this morning and cost two sessions a
round trip each. Its author is independently known — `ad379a7c` self-reported writing it, and
`59112612` and `5399543d` each got `UNKNOWN` from the tool. After the fix:

```
PEER      src/peer/server.rs
          written by 953b5e77-…  [not live — cannot be asked]
          written by ad379a7c-…  [LIVE]
                     fix-file-provenance-tool-blindness [idle] — pid 198818, profile .claude
                       ask it: SendMessage to="uds:/run/user/1000/cc-socks/198818.sock"
```

Both halves of the day's work composing: the widened write-detection found the author, and the
registry join (`14e07fb3`) made them reachable.

### A third narrowing, found by the fixture rather than by reading

The first cut passed a hand-written probe and then **failed** its own test case. `PY_ASSIGN` used
a bare `['"]`, but a transcript records a *shell-quoted* snippet, so the quotes routinely arrive
backslash-escaped — `Path(\"x\")`. `PY_WRITE_LITERAL` has always known this (`_Q = \\?['"]`); the
assignment resolver never did, so `p=\"lit\"` bound nothing. **That narrowing predates this bug**
and would have survived a fix aimed only at the Path-valued form. It is why the probe cases were
not enough: they were written with clean quotes, and the fixture is built from the recorded shape.

### Residual, re-derived with its own unit

Denominator stated deliberately: shell calls embedding a python snippet that **visibly writes**
(a write-construct token is present), so a read is not counted as a miss.

| lane | python-writing calls | unresolved before | unresolved after |
|---|---|---|---|
| `run_command` | 522 | 285 (54.6%) | 116 (22.2%) |
| `Bash` | 408 | 238 (58.3%) | 95 (23.3%) |
| **both** | **930** | **523 (56.2%)** | **211 (22.7%)** |

Of the 211 remaining: **130 f-string targets**, 24 computed (`argv`/`join`/call), 19
`dir / "name"`, 38 other. The f-string majority is **not a gap to close** — the path is not in the
snippet, so no static reading recovers it; case H is the fixture that pins it as an honest
`UNKNOWN` and is annotated inert so nobody credits it as coverage.

**This is not a delta on the 1.8%/2.9% figures, and must not be presented as one.** Those ran over
all shell calls at a different instant, counting every mutating verb rather than the python
subset; this scan's own totals are 40989 `run_command` and 8794 `Bash`, against their 21421 and
7516. Two honest counts of different populations. In *their* unit the fix newly resolves 0.41 pp
and 1.63 pp. Non-python verbs (`patch`, `perl -i`, `git apply`) are untouched and remain separate.

## Tests added

`tests/file-provenance.sh` 102 → 110. Five new positives (bare `var.write_text`, `open(var)` on a
`Path()`-bound name, `write_bytes`, module-qualified `pathlib.Path(…)`, and the escaped-quote
form) and four negatives — the negatives are the load-bearing half.

### The mutation run, and the three cases that exist only because of it

Seven mutations of the production path, **7 killed / 0 survivors** — but only after three rounds:

| mutation | first run | now |
|---|---|---|
| bare `var.write_` pattern removed | KILLED | KILLED |
| assignment loses the `Path()` branch | KILLED | KILLED |
| assignment loses escaped-quote tolerance | KILLED | KILLED |
| `write_` widened to ANY attribute | KILLED | KILLED |
| **`open()` mode check dropped → reads count** | **SURVIVED** | KILLED |
| **mode class widened `[wax]` → `[waxr]`** | **SURVIVED** | KILLED |
| **assignment narrowed to unqualified `Path(` only** | **SURVIVED** | KILLED |

- **The mode check survived because the pre-existing read-only case addresses `open()` with a
  LITERAL.** The variable path had no read-only guard at all, so making every `open(p)` a write
  killed nothing. *Mutate once per guarded SITE:* one law, two call sites, one guard.
- **Widening `[wax]` to `[waxr]` survived that fix too**, because the new case passes no mode
  argument at all. Absence of a mode and a read mode are different inputs; the mode class is the
  discriminator, so the character has to be guarded, not just the argument's absence.
- **`pathlib.Path(…)` survived** because every fixture used `from pathlib import Path`.

One further note on method: a fourth "survivor" was an artifact of the **mutation harness**, not a
coverage gap — the anchor `\s*\.write_"""` matches two patterns, so `replace(…, 1)` mutated the
wrong line and the run reported SURVIVED for a mutation never applied. The harness now asserts
`count(anchor) == 1` before mutating. A mutation run is itself an instrument, and an ambiguous
anchor is `IC-6`'s no-disambiguator half pointed at the tool doing the measuring.

Widen **both** narrowings, and keep the call as the discriminator:

1. Add `([A-Za-z_]\w*)\s*\.write_` — a bare variable receiving a write method.
2. Let the assignment resolver accept `VAR = Path("lit")` / `pathlib.Path("lit")` as well as
   `VAR = "lit"`.

**What must NOT be done, and it is the tempting move:** widen the *assignment* side to "any
assignment in the snippet" and treat every bound path as written. That makes every path a
snippet merely **reads** into an author, which is the mention-as-authorship failure this whole
tool exists to refuse. Case I is the regression guard for it — a `Path`-valued variable that is
only ever `read_text()` must stay out.

Then re-derive the residual as its own number with its own unit — the current
**1.8% of 21421 `run_command` calls / 2.9% of 7516 `Bash` calls** is a measurement of the
pre-fix matcher and will not be true afterwards.

## Notes

Case H (f-string) is correctly missed and should stay missed: the path is not in the snippet,
so no static reading can recover it. It belongs in the residual, not in the fix.

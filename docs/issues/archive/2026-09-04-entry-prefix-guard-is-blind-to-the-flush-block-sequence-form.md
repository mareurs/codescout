---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-04
opened: 2026-09-04
owner: marius
related: []
severity: high
---

# BUG: `declared_entry_prefixes` is blind to the flush block-sequence form, so both ledger guards silently do not run on 5 of 45 ledgers

## Summary

`entry_prefix:` followed by `- F` / `- W` at **zero extra indentation** is valid YAML and is what
five of this repo's ledgers actually use on disk — including `bug-fix-session-log.md`, the busiest
one. `declared_entry_prefixes` breaks out of its block-sequence branch on exactly that form and
returns `[]`, so those files read as *not a ledger*. Two protections are gated on that predicate and
therefore never run: `append_entry`'s cross-host high-water collision guard, and the librarian write
guard that puts a ledger off-limits to direct `edit_file`. The allocator's own reader parses the form
correctly, so the two readers disagree in precisely the direction their parity test was written to
prevent.

## Symptom (Effect)

Both guards were exercised live and both allowed. No error, no warning, no hint — the calls simply
succeeded:

```
# 1. unpushed-commit guard should have refused; it did not
doc(action="append_entry", id="2dd9d90bc83f9f49", id_prefix="W", ...)
  → {"id": "W-104", "section_written": true, "frontmatter_max": 103}

# 2. write guard should have refused a direct edit; it did not
edit_file(path="docs/trackers/bug-fix-session-log.md", edits=[...])
  → {"status": "ok", "wrote_to": "/home/marius/work/claude/codescout"}
```

At the moment of both calls, `f4e30856` — a commit touching that exact ledger file — was genuinely
unpushed:

```
$ git rev-parse --abbrev-ref @{upstream}
origin/experiments
$ git branch -r --contains f4e30856
                    # empty: on no remote branch
```

## Reproduction

1. `git rev-parse HEAD` → `0bc9679a` (branch `experiments`).
2. Take any ledger whose frontmatter uses the flush form:

   ```yaml
   entry_prefix:
   - F
   - W
   ```

3. Commit a change to that ledger without pushing.
4. Call `doc(action="append_entry", id=<ledger>, id_prefix="F", anchor_heading=…, title=…, body=…)`.
   Expected: `RecoverableError` naming *"Push this ledger's commits, then allocate."*
   Actual: the entry is allocated and written.
5. Call `edit_file` on the same file with any `old_string`/`new_string` pair.
   Expected: refusal — *"a declared `entry_prefix` puts this file off-limits to direct `edit_file`"*.
   Actual: `{"status": "ok"}`.

Indent the two sequence items by two spaces and both guards begin firing.

## Environment

codescout `experiments` @ `0bc9679a`, main checkout `/home/marius/work/claude/codescout`, Linux,
MCP over the release binary. Observed 2026-09-04 01:30–01:50 EEST.

## Root cause

`src/util/librarian_guard.rs:293-335`, the block-sequence branch of `declared_entry_prefixes`:

```rust
for next in lines {
    if next == "---" { break; }
    let t = next.trim_start();
    let Some(item) = t.strip_prefix("- ") else { break; };
    if next.len() == t.len() {
        // Not indented — a top-level sequence, so this key is not its parent.
        break;
    }
    out.extend(clean_prefix(item));
}
```

**The comment states a rule YAML does not have.** A block sequence whose items sit at the *same*
column as the parent key is standard, valid YAML — it is the style `serde_yml` emits and the style
every affected file on disk uses. `next.len() == t.len()` is true for a flush `- F`, so the loop
breaks on the first item and the function returns an empty vec. The caller then computes
`is_ledger = false` (`src/librarian/tools/append_entry.rs:152-155`) and skips the unpushed check
entirely; the same predicate backs the write guard's ledger arm.

The other reader — `declared_prefixes_from_frontmatter`, used by `allocate_entry_id` — goes through
`serde_yml` and parses the form correctly. That asymmetry is not inferred: the append at step 4
allocated `W-104` and wrote `entry_high_water_W: 104`, which `allocate_entry_id` only does for an
artifact it agrees declares an `entry_prefix`. **Both directions were observed on the same file in
the same minute.**

Measured 2026-09-04: `declared_entry_prefixes` read against every file under `docs/` declaring
`entry_prefix` → **5 of 45** return empty.

## Evidence

### The five guard-blind ledgers

```
docs/trackers/statement-validity-session-log.md
docs/trackers/cluster-promotion-session-log.md
docs/trackers/prompt-surface-compaction-session-log.md
docs/trackers/prompt-surface-measurement-session-log.md
docs/trackers/bug-fix-session-log.md
```

All five are `F-N`/`W-N` session logs — the ledger class with the highest append rate and the one
CLAUDE.md routes reconnaissance output into. The other 40 use a scalar, an inline flow list, or an
indented block sequence, and are protected.

### The parity test excludes the only form the corpus uses

`both_entry_prefix_readers_agree_on_every_yaml_form`
(`src/librarian/catalog/augmentation.rs:3089-3133`) enumerates **11** YAML forms. Its two
sequence cases are:

```rust
("block sequence",             "…entry_prefix:\n  - F\n  - W\n---…"),
("sequence then sibling key",  "…entry_prefix:\n  - F\n  - W\nentry_high_water_F: 3\n---…"),
```

Both indent by two spaces. The flush form is in neither, so the assertion is computed over a
population from which the failing member is absent — it passes, and would pass with the guard's
sequence branch deleted outright. The test's own doc comment names this exact outcome:

> a disagreement is silent in the dangerous direction: the allocator honours a form the guard is
> blind to, so entries in that ledger can be hand-written past the allocator with no error anywhere.

### The consequence the guard exists to prevent has already happened once

`declared_entry_prefixes`' own doc comment records that hand-written entry headings *"could be
hand-written straight past the allocator — which is how the R-N ledger came to reuse nine ids."*
That is the failure mode currently unguarded on five ledgers.

## Hypotheses tried

1. **Hypothesis:** the ledger's `abs_path` is stored relative, so `strip_prefix(workdir)` fails and
   `ledger_has_unpushed_commits` returns `false` on its every-failure-path-allows contract.
   **Test:** `sqlite3 -readonly ~/.local/share/librarian/catalog.db "select abs_path from artifact
   where id='2dd9d90bc83f9f49';"`
   **Verdict:** rejected — returns the absolute
   `/home/marius/work/claude/codescout/docs/trackers/bug-fix-session-log.md`. (`doc(action="find")`
   *displays* it relative, which is what suggested the hypothesis.)

2. **Hypothesis:** `ledger_has_unpushed_commits` fails to match the ledger's path in the revwalk.
   **Test:** read the function (`append_entry.rs:369-429`); it is never reached — the `is_ledger &&`
   short-circuit at `:155` is false first.
   **Verdict:** rejected — the guard is skipped, not wrong.

3. **Hypothesis:** the frontmatter is not really a block sequence (a bare key with the items
   belonging to something else).
   **Test:** `sed -n '1,14p'` on the file.
   **Verdict:** rejected — `entry_prefix:` / `- F` / `- W`, followed by `entry_high_water_F: 114`.
   `tags:` in the same block uses the identical flush style and parses fine everywhere else.

## Fix

Applied on `experiments` — the one-condition deletion prescribed above, not an inversion. The
block sequence is now bounded by the ITEM test alone:

```rust
let Some(item) = next.trim_start().strip_prefix("- ") else { break; };
```

The `if next.len() == t.len() { break; }` arm is gone. The sibling-key case it was believed to
protect is held by the item test, and that is now pinned by a fixture rather than argued.
## Tests added

Two cases added to `both_entry_prefix_readers_agree_on_every_yaml_form`
(`src/librarian/catalog/augmentation.rs`) — **not** a new test. The defect was the fixture list's
population, so a second test asserting the same agreement over the same 11 forms would have added
nothing.

- `"flush block sequence"` — the form the corpus actually uses. **RED observed before the fix**, in
  the diagnostic direction: `left: ["F", "W"]` (allocator) vs `right: []` (guard), both readers on
  one input.
- `"flush sequence then sibling key"` — pins that the item test is what now stops
  `entry_high_water_F: 3` being swallowed, since the indentation test believed to do that is gone.

Both fixture lines carry an annotation naming what breaks if the indentation is "tidied" back in.

**Not** added: the corpus-derived case proposed above. It remains the stronger guard, but it is a
different test with a different failure mode — it goes red when a ledger is *edited*, not when the
parser regresses. Left as an explicit follow-up rather than folded in silently.

Still overclaiming: `every_yaml_form_of_entry_prefix_is_recognised`
(`src/util/librarian_guard.rs`) passed both before and after this fix, so it carries the same
fixture omission under an even broader name. The parity test now covers the form; that one does not.
## Fix provenance

Fixed on `experiments` — the indentation test deleted, not inverted.

- **SHA:** `56f3d0bb`
- **patch-id:** `aaeeeacaaa4ae26e7eaa0057a9671520c95989dd`

Gate green at fix time: `cargo fmt -- --check` clean; `clippy --workspace --all-targets --features
local-embed -D warnings` clean — re-run after `touch` on both files, because the first pass returned
in 0.33s off a warm cache and a cache hit is not evidence; default lane 8657 passed / 1 failed. That
one failure is `peer::server::tests::run_exits_after_idle_timeout_with_no_connections`, itself open
as `docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`, in
`peer/server.rs`, and it passes in 1.15s run alone.

**The lean lane's `exit 0` is NOT evidence for this fix.** Measured 2026-09-04 by running the
`entry_prefix` filter through both lanes and reading the test NAMES rather than either total:

| lane | `entry_prefix` tests run | the new regression guard |
|---|---|---|
| `--no-default-features` | 3 — all `util::librarian_guard::tests::*` | **absent** |
| default | 6 | `librarian::catalog::augmentation::tests::both_entry_prefix_readers_agree_on_every_yaml_form` |

The parity test lives under `librarian::`, which `--no-default-features` compiles out, so the lean
lane never built the regression guard at all. And it is worse than merely thin: the one
`entry_prefix` test the lean lane DOES run is `every_yaml_form_of_entry_prefix_is_recognised`, which
passed **before and after** this fix — so on the lean lane the broken parser and the fixed one are
indistinguishable. This file is a datapoint for CLAUDE.md § *Development Commands*, whose
lean-lane-vacuity rule was added the same day.

**Scope of the verification, stated because it is narrower than "fixed" sounds.** The parser and the
two readers' agreement are verified, in the default lane. The live end-to-end effect named under
*Resume* — that `append_entry` now refuses on an unpushed `bug-fix-session-log.md` — is **not**: the
running MCP server predates this commit, so confirming it needs `cargo rb` and an `/mcp` reconnect.
## Workarounds

Re-indent the five files' sequence items by two spaces — a whitespace-only change that restores both
guards immediately and does not touch the allocator's behaviour. Until then, treat those five
ledgers as unguarded: do not hand-write `## PREFIX-N` headings into them, and push before appending.

## Resume

Done: fixture added, RED observed, indentation arm deleted, gate green, committed at `56f3d0bb`.

Remaining, and only this — `cargo rb`, then `/mcp` reconnect, then confirm the live effect on a
ledger using the flush form: commit a change to `docs/trackers/bug-fix-session-log.md` without
pushing and check `append_entry` refuses with *"Push this ledger's commits, then allocate."* Until
that runs, the fix is verified at the parser and unverified at the tool surface.
## References

- `src/util/librarian_guard.rs:293-335` — `declared_entry_prefixes`, the mis-parse.
- `src/librarian/tools/append_entry.rs:152-155` — the `is_ledger &&` short-circuit.
- `src/librarian/catalog/augmentation.rs:3089-3133` — the parity test whose fixtures exclude the form.
- `docs/trackers/bug-fix-session-log.md` — `F-114` / `W-104`, appended during the session that found this.
- `get_guide("tracker-conventions")` § *Make the tracker guarded* — states that any YAML form works,
  which is the documented contract this breaks.

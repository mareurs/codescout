---
kind: bug
status: open
tags:
- cluster/guard-narrower-than-its-name
closed: null
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

Not applied. The change is one condition in `declared_entry_prefixes`: a flush `- ` line following
`entry_prefix:` is an item of that key's sequence, not a top-level sequence. The existing
sibling-key guard (`strip_prefix("- ")` failing ends the loop) already stops `entry_high_water_F: 3`
from being swallowed, so the indentation test is doing no work the item test does not already do —
**delete it rather than invert it**, and confirm against the `sequence then sibling key` case.

**Do the test first, and add the case to the parity test rather than a new one** — a fixture list
that omits the corpus's only real form is the actual defect here; a second test asserting the same
agreement over the same 11 forms adds nothing. RED must be observed on the flush case before the
one-line change.

## Tests added

None yet. The regression guard is the flush block-sequence case added to
`both_entry_prefix_readers_agree_on_every_yaml_form` — where an observed RED is available, since the
two readers demonstrably disagree there today.

Worth pairing with a corpus-derived case: the eleven fixtures were written from the YAML spec, and
the form that broke is the one the repo's own writers emit. A test that reads the actual
`entry_prefix:` blocks under `docs/` and asserts both readers agree on each would have failed on day
one.

## Workarounds

Re-indent the five files' sequence items by two spaces — a whitespace-only change that restores both
guards immediately and does not touch the allocator's behaviour. Until then, treat those five
ledgers as unguarded: do not hand-write `## PREFIX-N` headings into them, and push before appending.

## Resume

Read `declared_entry_prefixes` (`src/util/librarian_guard.rs:293-335`), add
`("block sequence, flush", "---\nkind: tracker\nentry_prefix:\n- F\n- W\n---\n\n# L\n")` to the
fixture list in `both_entry_prefix_readers_agree_on_every_yaml_form`
(`src/librarian/catalog/augmentation.rs:3089`), observe RED, then delete the
`if next.len() == t.len() { break; }` arm and re-run. Verify the live effect by committing a change
to `docs/trackers/bug-fix-session-log.md` without pushing and confirming `append_entry` now refuses
with *"Push this ledger's commits, then allocate."*

## References

- `src/util/librarian_guard.rs:293-335` — `declared_entry_prefixes`, the mis-parse.
- `src/librarian/tools/append_entry.rs:152-155` — the `is_ledger &&` short-circuit.
- `src/librarian/catalog/augmentation.rs:3089-3133` — the parity test whose fixtures exclude the form.
- `docs/trackers/bug-fix-session-log.md` — `F-114` / `W-104`, appended during the session that found this.
- `get_guide("tracker-conventions")` § *Make the tracker guarded* — states that any YAML form works,
  which is the documented contract this breaks.

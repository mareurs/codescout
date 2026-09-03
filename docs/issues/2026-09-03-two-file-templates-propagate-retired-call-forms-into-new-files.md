---
status: open
opened: 2026-09-03
closed:
severity: medium
owner: marius
related: []
tags: ["cluster/selector-narrower-than-its-population"]
kind: bug
---

# BUG: four more instruction surfaces carry retired call forms, and two of them are templates that propagate into files created after the fix

## Summary

**This is a supplement to `docs/issues/2026-09-03-retired-tool-names-survive-in-the-surfaces-that-actually-reach-agents.md`
(artifact `bd0979bf7e454567`, severity high), not an independent finding.** That file
establishes the class and the site; do not read this one first. It adds four verified
surfaces its fix sketch does not list, and one of the four changes the fix's shape rather
than only its size: `docs/issues/_TEMPLATE.md` and `docs/templates/session-log.md` are
**copied to create new files**, so their retired call forms keep entering the repo after
every instance has been swept.

It is filed separately rather than appended because `bd0979bf7e454567` is **untracked in the
main checkout** — a live peer session's in-flight working tree — and writing into it would be
the interference recorded in
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`.

## Symptom (Effect)

Four surfaces, 16 occurrences, none named in `bd0979bf7e454567`'s fix sketch (whose step 4
extends the gate to `.codescout/memories`, `.codescout/private-memories` and
`docs/issues/_TEMPLATE.md` only):

| occurrences | file | what it is |
|---|---|---|
| 9 | `docs/TEAM-ONBOARDING.md` | the onboarding guide; its own line 193 row is "Known bugs before I re-discover one" |
| 4 | `docs/ROADMAP.md` | current status, present tense |
| 2 | `docs/templates/session-log.md` (`:12`, `:240`) | **copied to create every new session log** |
| 1 | `docs/issues/_TEMPLATE.md:92` | **copied to create every new bug file** — `:20` is already listed there, `:92` is not |

```
docs/templates/session-log.md:12:> artifact(action="append_entry", id="<artifact id>", id_prefix="F",
docs/templates/session-log.md:240:     artifact(action="append_entry", id="<artifact id>", id_prefix="F",
docs/issues/_TEMPLATE.md:92:Archive via artifact(action="move", id=..., new_rel_path="docs/issues/archive/...")
```

## Reproduction

At `d864c46f` (worktree `.worktrees/bug-claim-liveness`, rebased onto `experiments`
`26b1f5c6`):

```
cargo test --lib reader_docs_contain_no_retired_call_forms   # passes
for f in docs/TEAM-ONBOARDING.md docs/ROADMAP.md docs/templates/session-log.md \
         docs/issues/_TEMPLATE.md; do
  printf '%4d  %s\n' "$(grep -o -E 'artifact\(|artifact_event\(|artifact_augment\(|artifact_refresh\(|read_markdown\(|edit_markdown\(' "$f" | wc -l)" "$f"
done
```

## Environment

Linux. Branch `bug-claim-liveness` at `d864c46f`. Gate green at filing: fmt 0, clippy 0, lean
3512 passed / 0 failed, default 5340 passed / 0 failed.

## Root cause

The same one `bd0979bf7e454567` § *Why the gates missed it — the class* names, and its wording
stands. What this file adds is a **direction the ranking heuristic does not cover.** That file
prescribes ranking candidate surfaces by *how often an agent reads them without asking*, which
correctly puts the auto-loaded memory store first. A template is read rarely — but every read
**produces a new file containing the defect**. Read-frequency and propagation are different
axes, and the two templates score low on the first and highest possible on the second.

Derivation of the coverage figure, so it is re-checkable rather than quotable: classifying all
1516 git-tracked `*.md` files by the gate's own `ROOTS`/`FILES` predicate
(`src/prompts/mod.rs:2062-2079`) gives **152 files scanned, 10%**, with 0 in-scope occurrences
and 2163 out-of-scope ones across 413 files. **The 2163 is not a defect count and must not be
quoted as one** — the overwhelming majority are historical records (session logs, archived
issues, plans) that the gate's doc comment `:2030-2042` deliberately and correctly tolerates.
The defect is the live-instruction-surface subset, which is what both this file and
`bd0979bf7e454567` enumerate by hand. Unit throughout: occurrences, not lines, matching the
gate's per-`(line, call)` `bad` vector.

## Evidence

### The two templates propagate

`docs/issues/_TEMPLATE.md:92` is the archive instruction every new bug file inherits:

```
Archive via artifact(action="move", id=..., new_rel_path="docs/issues/archive/...")
— never a bare git mv, which orphans the catalog row.
```

The advice around the dead call is still correct — the catalog-vs-`git mv` point holds — which
makes this worse than a plainly broken instruction: the surrounding text lends the dead call
form credibility. This bug file's own creation is the reproduction; the template was copied to
produce it.

### The gate's doc comment cites the wrong cluster

`src/prompts/mod.rs:2029` reads *"That is `IC-11` exactly"*. `IC-11`
(`docs/trackers/issue-clusters.md:312`) is *"documentation denies a capability the code has
since gained"*, slug `doc-contradicted-by-code`. The sentence that follows the citation
describes `IC-18`, *"a selector is narrower than the population it names"* — accurately, it
just cites the wrong id, so a reader following the pointer lands on an unrelated class. Fixed
separately as a one-token correction; recorded here because that is where it was noticed.

## Hypotheses tried

1. **Hypothesis:** these are lines the `bug-claim-liveness` branch added and failed to sweep.
   **Test:** `git grep -n -E '<retired forms>' 26b1f5c6 -- <files>`.
   **Verdict:** rejected. All present at the merge base. The branch's own five added lines were
   swept in `d864c46f`, and `src/prompts/guides/tracker-conventions.md` — the one file it
   touched that lies *inside* `ROOTS` — has zero residue at base. The residue sits exactly
   where the gate does not look.

2. **Hypothesis:** this duplicates `bd0979bf7e454567`.
   **Test:** read that file's `## Fix sketch` and `## Why the gates missed it — the class`.
   **Verdict:** **partially confirmed, and the file was rewritten because of it.** An earlier
   draft of this bug re-derived the class independently and was discarded. The class, the site
   and the severity belong to `bd0979bf7e454567`. Only the four surfaces above and the
   propagation axis survive as additive. Recorded rather than silently dropped, because the
   discarded draft is what a second reader would otherwise write again.

3. **Hypothesis:** the two 2026-09-02 gate-scope bugs in the `tool-collapse` worktree cover it.
   **Test:** read
   `2026-09-02-docs-architecture-is-in-neither-the-gates-inclusion-list-nor-its-exclusion-rationale.md`
   and `2026-09-02-both-doc-citation-guards-skip-half-the-corpus-without-saying-so.md`.
   **Verdict:** rejected as duplicates, kept as **class precedent**. Both are against
   `tests/doc_tool_refs.rs` — the *older* gate, the one
   `reader_docs_contain_no_retired_call_forms` was written to backstop. The replacement gate
   reproduced its predecessor's defect at a new site one day later: the remedy for an
   enumerated allowlist was another enumerated allowlist. Note how nearly this check failed —
   `doc(action="find", kind="bug")` from this worktree returns neither file, because they are
   worktree-scoped rows under a *different* worktree. They surfaced only via
   `librarian(action="doctor")`'s `worktree_scoped_row` list.

## Fix

Fold these four surfaces into `bd0979bf7e454567`'s step 4 — they are the same edit. But note a
disagreement worth settling before that step is executed, because it is the difference between
fixing instances and fixing the class:

- **Step 4 as written extends the allowlist by hand.** That is the third hand-extension of a
  scope on this gate lineage in two days, and hypothesis 3 above shows what the previous two
  bought.
- **The alternative is to invert the selector:** scan **all** tracked `*.md`, and maintain an
  explicit *exclusion* list of historical-record roots (`docs/issues/archive`, `docs/archive`,
  `docs/trackers`, `docs/superpowers`, `docs/evals`, `CHANGELOG.md`). A new file is then covered
  by default, and every exemption is a written claim that the file is a record. The cost is
  honest and non-trivial: the exclusion list is itself hand-written, and the 2163 out-of-scope
  occurrences need classifying once. What changes is the failure mode — from *silently
  unscanned* to *loudly scanned until someone justifies an exemption*, which is the shape
  `CLAUDE.md` § *Observer Blindness* asks for.

Separately: `src/prompts/mod.rs:2029` `IC-11` → `IC-18`.

SHA: *pending.* patch-id: *pending.*

## Tests added

None yet. For whichever direction is chosen, the non-vacuity pattern at
`src/prompts/mod.rs:2094-2103` should extend to `FILES` as well as `ROOTS` — every named file
must exist, so a rename cannot silently empty an entry. That guard exists for roots today and
not for files.

## Workarounds

When copying `docs/issues/_TEMPLATE.md` or `docs/templates/session-log.md`, translate by hand:
`artifact(action="find"…)` → `doc(action="find"…)`, `artifact(action="move"…)` →
`doc(action="move"…)`, `artifact(action="append_entry"…)` → `doc(action="append_entry"…)`.

Treat a green `reader_docs_contain_no_retired_call_forms` as covering 152 of 1516 markdown
files; read `src/prompts/mod.rs:2062-2079` for which.

## Resume

Do not act on this file alone — read `bd0979bf7e454567` first and settle the allowlist-vs-
exclusion-list question in its `## Fix sketch` step 4, since that decides whether these four
surfaces are added by hand or covered by default.

**Catalog note:** the `cluster/selector-narrower-than-its-population` tag is in this file's
frontmatter, which for a *new* file is the catalog's source — the classifier reads it on first
index, the same way `kind: bug` is discovered. BL-48 (a frontmatter edit not reaching the
catalog) applies to files that *already* have a row, not to this one. It has no row yet because
it was created in the `bug-claim-liveness` worktree, where `librarian(action="reindex")` walks
zero files — see
`docs/issues/2026-09-03-reindex-walks-zero-files-in-a-worktree-and-reports-success.md`. Verify
the tag landed after the branch reaches the main checkout and is reindexed there. The cluster
differs deliberately from `bd0979bf7e454567`'s `cluster/guard-narrower-than-its-name` (`IC-14`):
that file's claim is about the guard's **name** overstating its coverage, this one's is about
the **selector** being narrower than its population (`IC-18`).

## References

- **Primary record:** `docs/issues/2026-09-03-retired-tool-names-survive-in-the-surfaces-that-actually-reach-agents.md`,
  artifact `bd0979bf7e454567`, severity high, tagged `cluster/guard-narrower-than-its-name`.
- `src/prompts/mod.rs:2018-2118` — the gate. `ROOTS` `:2062-2068`, `FILES` `:2069-2079`,
  per-root non-vacuity `:2094-2103`, the `IC-11` miscitation `:2029`.
- [`docs/trackers/issue-clusters.md`](../trackers/issue-clusters.md) — `IC-18`
  (`cluster/selector-narrower-than-its-population`), artifact `1b5a080fe2efcb6b`; its mechanism
  column already reads "partial — nothing reaches an author-written selector".
- Class precedents, both open in the `tool-collapse` worktree:
  `docs/issues/2026-09-02-docs-architecture-is-in-neither-the-gates-inclusion-list-nor-its-exclusion-rationale.md`
  and `docs/issues/2026-09-02-both-doc-citation-guards-skip-half-the-corpus-without-saying-so.md`.
- `d864c46f` — the commit that swept this branch's own five added lines, whose green gate is
  the symptom above.

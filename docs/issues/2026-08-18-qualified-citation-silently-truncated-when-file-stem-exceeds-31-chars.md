---
status: fixed
opened: 2026-08-18
closed:
severity: medium
owner: marius
related: []
tags: [librarian, link-scan, citations]
kind: bug
---

# BUG: a qualified citation is silently truncated when the file stem exceeds 31 characters

> **Status: fixed** — root cause identified, one-line parser fix, regression test
> mutation-verified in an isolated worktree. Not yet archived: the full gate and the
> live-MCP verification are blocked on an unrelated concurrent build failure. See
> **Resume**.
## Summary

`link_scan`'s cross-repo token regex caps the qualifier at 31 characters. A citation
qualified by a longer file stem still matches — starting from a word boundary partway
through the stem — so it resolves to a **different, shorter qualifier that names no file
in the repo**, is classified `cross_repo`, and **never becomes an edge**. It fails
silently: `cross_repo` is a legitimate category, so the report reads as intentional.

This defeats the exact remedy `get_guide("tracker-conventions")` prescribes for the
ambiguous-`F-N` problem, on precisely the ledgers that need it most.

## Symptom (Effect)

Citing `prompt-surface-compaction-session-log:F-4` (a file that exists, at
`docs/trackers/prompt-surface-compaction-session-log.md`) from
`docs/trackers/prompt-hamsa-audit-log.md`:

```json
"cross_repo": [
  {
    "src_id": "59ebeebb6ed05c89",
    "raw": "surface-compaction-session-log:F-4",
    "kind": "CrossRepoToken",
    "line": 870
  }
]
```

Note `raw` — the leading `prompt-` is **gone**. `edges_missing` stays `0` and nothing
is reported as dangling or ambiguous, so the run looks clean.

## Reproduction

1. From any artifact, cite `prompt-surface-compaction-session-log:F-4` in prose.
2. `librarian(action="link_scan", write=true)`.
3. Read `cross_repo` — the entry appears with a truncated `raw`, and no `cites` edge is
   created to `03464a8808345846`.

Bare `F-4` instead produces an **ambiguous** entry (10 candidate definers), so neither
citation form works for this ledger. Commit: `338f8ea7`, branch `experiments`.

## Environment

codescout 0.15.0, branch `experiments`, Linux. Not transport-specific — the regex is
compile-time.

## Root cause

`src/librarian/tools/link_scan/extract.rs:98`:

```rust
RE.get_or_init(|| Regex::new(r"\b[a-z][a-z0-9_-]{1,30}:[A-Z]{1,3}-\d+\b").unwrap())
```

The qualifier is `[a-z][a-z0-9_-]{1,30}` — one leading character plus at most 30 more,
so **31 characters maximum**. `prompt-surface-compaction-session-log` is 37.

The match does not fail; it *slides*. `-` is a non-word character, so there is a word
boundary before every hyphen-separated segment. The engine finds the leftmost position
from which the whole pattern matches — here the `s` of `surface` — and captures
`surface-compaction-session-log` (30 chars), which fits. Resolution then looks for a
file with that stem, finds none, and correctly classifies an unresolvable qualifier as
cross-repo. Every step behaves as written; the cap is the defect.

*measured 2026-08-18: `librarian(action="link_scan", write=true)` on `experiments` at
`338f8ea7` → the entry above, with `edges_missing: 0`.* Stem lengths measured the same
day by `for f in docs/trackers/*.md docs/issues/*.md; do …; done`.

## Evidence

### Blast radius — stems over the cap

32 files under `docs/trackers/` + `docs/issues/` exceed 31 characters. Bug files are
structurally guaranteed to (`YYYY-MM-DD-` is 11 characters before the slug starts), but
they own no entry namespace, so they are not the affected population.

The population that matters is **ledgers**, and today exactly one is over:

| stem | chars | `entry_prefix`? |
|---|---:|---|
| `prompt-surface-compaction-session-log` | 37 | **yes** (`[F, W]`) |
| `worktree-semantic-search-session-log` | 36 | no |
| `structural-edit-gate-session-log` | 32 | no |
| `local-onnx-embedding-session-log` | 32 | no |

The margin is thin, which is why this is worth fixing rather than renaming around:
`-session-log` alone is 12 characters, so any topic of 20+ characters trips the cap, and
`structural-edit-gate-session-log` misses by exactly one.

### Why the failure is silent

`cross_repo` is documented as legitimate — *"A qualifier naming no file in this repo is
still a cross-repo reference (`codescout:A-11`): reported, never turned into an edge."*
So a truncated qualifier lands in a bucket that means "working as intended", and
`edges_missing: 0` corroborates it. Nothing distinguishes a deliberate `repo:TOKEN`
reference from a same-repo citation the regex ate.

## Hypotheses tried

1. **Hypothesis:** the qualified form is not recognised at all and falls back to a bare
   token. **Test:** read `raw` in the `cross_repo` array. **Verdict:** rejected — it *is*
   recognised, as `CrossRepoToken`, just with a truncated qualifier.
2. **Hypothesis:** the backticks around the citation break the match. **Test:** the
   captured `raw` starts mid-stem rather than being absent, so backticks are irrelevant
   (`` ` `` is a non-word char and only supplies another boundary). **Verdict:** rejected.
3. **Hypothesis:** the 31-character cap in `cross_repo_re`. **Test:** count the stem (37)
   against the cap (31), and check the captured substring is exactly the longest
   boundary-aligned suffix that fits (30). **Verdict:** confirmed.

## Fix

`src/librarian/tools/link_scan/extract.rs` — qualifier bound raised from `{1,30}` (31
chars) to `{1,119}` (120), with the slide mechanism written into the doc comment so the
next person to tighten it knows what a too-small bound actually does.

120 clears every stem in the repo (longest today: 90) with headroom. It cannot over-match
into prose: the pattern still requires `:` immediately followed by an entry token, so a
larger allowance only extends a run of `[a-z0-9_-]` already adjacent to a `:PREFIX-N`.

Considered and rejected: renaming the ledger to fit. That trades a one-line parser fix
for a rename that re-keys the artifact (`id = sha256(abs_path)`), breaks every existing
citation of its path and id, and leaves the cap in place for the next author.

Fix SHA: see the commit that archives this file (branch **`experiments`**).
## Tests added

`long_file_stem_qualifier_is_captured_whole_not_truncated_to_a_suffix` —
`src/librarian/tools/link_scan/extract.rs` (tests module, after
`cross_repo_token_masks_embedded_entry_token`).

It asserts on the captured **string**, not a count. That distinction is the whole test:
a count-only assertion passes on the truncated capture, which is exactly how the defect
survived in the first place.

**Mutation-verified by applying the mutation and observing the result**, not by reasoning
about coverage. In an isolated worktree at `1281d9ac`, reverting the bound to `{1,30}`
failed the test with the defect reproduced verbatim:

```
left:  ["surface-compaction-session-log:F-4"]
right: ["prompt-surface-compaction-session-log:F-4"]
```

Restoring `{1,119}` passed. 46/46 `link_scan` tests green in that worktree.

The worktree was necessary rather than incidental: the main checkout's `cargo test` was
red at the time on 64 uncommitted insertions in `src/tools/guide_ledger.rs` from a
concurrent session (tests referencing `rendezvous_active` / `set_rendezvous_active`,
methods not yet written). Unrelated to this change and not swept.
## Workarounds

Cite by **rel_path** instead of a qualified entry token
(`docs/trackers/prompt-surface-compaction-session-log.md`), which resolves and creates an
edge. This is what A-27's *Related* line does for the surface measurement.

## Resume

Root cause, fix and regression test are all landed and verified in isolation. What is
**not** yet done, and why this file is not archived:

1. Full `cargo test` gate green in the main checkout — currently blocked by the
   concurrent session's in-flight `src/tools/guide_ledger.rs` edits, not by anything
   here. Re-run once that tree compiles.
2. `cargo rb` + `/mcp` reconnect. **`librarian(action="link_scan")` runs inside the live
   MCP server**, which is still the pre-fix build — so the fix is not observable through
   the tool until the release binary is rebuilt. Do not read a stale `cross_repo` row as
   evidence the fix failed.
3. Then re-run `librarian(action="link_scan", write=true)` and confirm A-27's *Related*
   line in `docs/trackers/prompt-hamsa-audit-log.md` moves out of `cross_repo` and
   materializes a `cites` edge to `03464a8808345846`.
## References

- `src/librarian/tools/link_scan/extract.rs:98` — the regex
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified*
- `docs/trackers/prompt-hamsa-audit-log.md` A-27 — the citation that surfaced it
- `docs/trackers/prompt-surface-compaction-session-log.md` — the affected ledger

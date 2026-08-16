---
id: a2899c126f1e7771
kind: bug
status: fixed
title: 'BUG: the librarian guard is keyed on YAML quoting, so 15 of 27 trackers are unprotected'
tags:
- librarian
- guard
- edit_markdown
- read_markdown
- data-loss
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md
severity: high
---

# BUG: the librarian guard is keyed on YAML quoting, so 15 of 27 trackers are unprotected

## Summary

`is_librarian_artifact` decides whether a file is librarian-managed by pattern-matching
the **frontmatter text** for an `id:` whose value is exactly 16 lowercase-hex characters.
A YAML-quoted id (`id: '9a892c2a5976e296'`) is 18 characters, fails the test, and the file
reads as unmanaged — so `read_markdown`, `edit_markdown` and `edit_file` all operate on it
directly. Whether a managed artifact is protected therefore depends on a serialisation
choice no author made deliberately. In `docs/trackers/` alone, **15 of 27 are
unprotected**, including the active work queue.

## Symptom (Effect)

Two `kind: tracker` artifacts, both augmented, both librarian-managed. Same tool, same
directory, opposite outcomes:

```
read_markdown("docs/trackers/tool-usage-patterns.md")
-> ERROR: 'docs/trackers/tool-usage-patterns.md' is a librarian-managed artifact
          — do not read or edit it directly

read_markdown("docs/trackers/open-issue-work-queue.md")
-> 285 lines  @file_0a6aab4d          <- allowed
```

The only difference between the two files:

```
tool-usage-patterns.md:   id: abc513d3ee0f0b50
open-issue-work-queue.md: id: '9a892c2a5976e296'
```

## Reproduction

On `experiments` at `4b77dff5`, via the live MCP server:

```
read_markdown(path="docs/trackers/open-issue-work-queue.md")   # succeeds, should refuse
read_markdown(path="docs/trackers/tool-usage-patterns.md")     # refuses, correctly
```

## Environment

Linux, `experiments`, `4b77dff5`, live MCP server (not a unit test — observed through the
real tool surface).

## Root cause

`src/util/librarian_guard.rs:31-45`:

```rust
if let Some(val) = line.strip_prefix("id: ") {
    let val = val.trim();
    return val.len() == 16 && val.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
}
```

`'9a892c2a5976e296'` has `len() == 18` because the quotes are part of the value as read
here, so the predicate is false and the function returns `false` on the first `id:` line
it sees. The guard never consults the catalog — it is a text heuristic over frontmatter,
not a lookup of whether this path *is* a managed artifact.

Two independent failure modes follow from the same design:

- **False negative (the reported one):** a quoted id disables the guard entirely.
- **False positive by luck:** a *stale* id keeps the guard on, because only the shape is
  checked. `tool-usage-patterns.md` asserts `abc513d3ee0f0b50`, which resolves to
  **nothing** — its live catalog id is `f2ecdd76a6189efb`. It is protected by accident.

*Measured 2026-08-16: `artifact(find, filter={"id":{"in":["abc513d3ee0f0b50",
"f2ecdd76a6189efb"]}}, include_archived=true)` returns exactly one row, the latter.
Guard body read via `symbols(name="is_librarian_artifact", include_body=true)`.*

### Correction to the numbers above, and to half the diagnosis

Two things in the original filing were wrong, and a first attempt at this section was
wrong again in the other direction. Recording all three, because the sequence is the
lesson.

**The denominator.** `docs/trackers/` holds **41** markdown files, not 27. The 12/15
split counts only files carrying a 16-hex `id:` at all; **14 more carry none**. Accurate
statement: of 27 trackers with an id, 15 were unprotected by quoting. Repo-wide the
quoting fix newly guards **86** files, not 15.

**First correction, itself wrong.** I initially concluded the id-less files were not a
defect — that the stamped `id:` was an intentional opt-in marker separating artifacts the
librarian *wrote* from prose trackers it merely catalogues, and that a catalog-backed
guard would break the documented `edit_markdown` workflow on
`docs/trackers/skill-frictions.md`. That reasoning deferred to a convention instead of
testing it. The convention is not the specification.

**What the measurement actually shows.** Two queries settle it:

```
artifact(find, filter={rel_path contains "docs/RELEASE.md"|"CONTRIBUTING"|...})
-> docs/RELEASE.md, CONTRIBUTING.md, docs/PROGRESSIVE_DISCOVERABILITY.md,
   docs/TAXONOMY.md, docs/ROADMAP.md, src/prompts/README.md   — ALL catalog rows

artifact(find, augmented=true, scope="repo")   -> count: 16
```

So **catalog membership is the wrong predicate** — 500+ rows, essentially all of `docs/`;
a guard keyed on it would refuse `read_markdown("docs/RELEASE.md")`. But **augmentation is
the right one**: exactly 16 artifacts repo-wide, all trackers, and precisely the set where
state lives *outside* the file. It lands correctly on every case that mattered:

| file | augmented | direct edit |
|---|---|---|
| `open-issue-work-queue.md`, `tool-usage-patterns.md` | yes | refused |
| `skill-frictions.md`, `reconnaissance-patterns.md` | no | **allowed** — as CLAUDE.md documents |
| `RELEASE.md`, `CONTRIBUTING.md`, archived bug files | no | allowed |

The documented workflow survives — but now because it is *correct*, not because it is
written down. And the id heuristic, even fixed for quoting, still misses one genuinely
dangerous file: `docs/trackers/artifact-augmentation-followups.md` is augmented **and**
carries no frontmatter id.

**The plumbing objection was real but not decisive.** The core `ToolContext`
(`src/tools/core/types.rs:58-80`) that `read_markdown`, `edit_markdown` and `edit_file`
receive genuinely has no catalog handle — only the librarian's
(`src/librarian/tools/mod.rs:84-103`) does — and adding a field to it means touching
**124 construction sites across 21 files**. The way through is a narrow trait object
installed once at server construction, which costs one line in `server.rs` and no
churn anywhere else.
## Evidence

### More trackers are unprotected than protected

```
$ grep -rlE "^id: [0-9a-f]{16}$" docs/trackers/*.md      # protected
docs/trackers/code-dupes-backlog.md
docs/trackers/doc-ref-audit.md
docs/trackers/fable-tuning-index.md
docs/trackers/fable-tuning-research.md
docs/trackers/fable-tuning-tasks.md
docs/trackers/legibility-backlog.md
docs/trackers/provenance-probe-session-log.md
docs/trackers/provenance-subsystem.md
docs/trackers/pr-review-session-log.md
docs/trackers/release-promotion-session-log.md
docs/trackers/tool-usage-patterns.md
docs/trackers/tracker-discovery-semantic-eval.md
                                                          # 12

$ grep -rlE "^id: '[0-9a-f]{16}'$" docs/trackers/*.md    # UNPROTECTED
docs/trackers/2026-08-15-tool-usage-investigation.md
docs/trackers/archived-bug-sha-reconciliation.md
docs/trackers/capability-proposals.md
docs/trackers/dependency-review-session-log.md
docs/trackers/fable-tuning-findings.md
docs/trackers/local-onnx-embedding-session-log.md
docs/trackers/open-issue-work-queue.md
docs/trackers/prompt-hamsa-audit-log.md
docs/trackers/retrieval-benchmark.md
docs/trackers/run-command-pipeline.md
docs/trackers/structural-debt-refactor.md
docs/trackers/test-escape-hardening.md
docs/trackers/tracker-hygiene-log.md
docs/trackers/tracker-management-redesign.md
docs/trackers/windows-platform-support.md
                                                          # 15
```

Both listings ran under `head -30` and returned 12 and 15 lines, so neither was truncated.

### It reframes an earlier finding

`docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md`
established that 50 files repo-wide carry an unquoted 16-hex `id:` while 141 carry a
quoted or `null` one. That was recorded as a *coverage* measurement. It is really a
*protection* measurement: those 141 files are not merely uncounted, they are **unguarded**,
and nothing about their content distinguishes them from the 50.

### The guard's own hardening did not touch this

The `edit_file` fix (`47abcb6d`) hoisted `guard_not_librarian_managed` into the shared
read so all three write paths call it. Correct, and orthogonal: every path now asks the
same question, and for 15 of 27 trackers the answer is still `false`.

## Hypotheses tried

1. **Hypothesis:** the two trackers differ in augmentation state, and the guard keys on
   that.
   **Test:** both are `kind: tracker` with an `entry_collection`; both answer
   `append_entry` / `update_entry`.
   **Verdict:** rejected — the guard never reads the catalog at all.

2. **Hypothesis:** `strip_prefix("id: ")` mis-parses the quoted form into something odd.
   **Test:** read the body; the value is taken verbatim after `id: ` and trimmed.
   **Verdict:** rejected — parsing is fine, the *length test* is what fails. `'…'` is 18.

## Fix

Implemented 2026-08-16 on `experiments`. The guard now asks **two independent questions**,
and refuses if either says yes. Neither implies the other: a stamped id says the librarian
*wrote* this file; augmentation says the file is not where its state *lives*.

**1. Frontmatter id, quoting-insensitive.** `is_librarian_id` strips one layer of matching
`'` or `"` before the 16-lowercase-hex test, and the key match loosened from
`strip_prefix("id: ")` to `strip_prefix("id:")` + `trim()`. A mismatched or unterminated
pair is left alone, so a malformed value fails the hex test rather than being coerced
through it. Closes 86 files repo-wide.

**2. Augmentation, via the catalog.** New `AugmentedArtifactOracle` trait in
`src/util/librarian_guard.rs`, implemented in `src/librarian/adapter.rs` as
`CatalogAugmentationOracle` (`artifact_id_from_abs` → `augmentation::get`, a primary-key
hit) and installed once at server construction. Left uninstalled — tests, or a build
without the librarian — the guard degrades to question 1 rather than failing open.

Design notes worth keeping:

- **Why a process-global `OnceLock` rather than a `ToolContext` field.** There is one
  catalog per process; adding a field to the core `ToolContext` means editing 124
  construction sites. Precedent for process-global mutable state is `src/heartbeat.rs`.
- **Why the decision function takes the oracle explicitly.** `guard_with_oracle` is the
  testable core; the public wrapper reads the global. No test ever installs into the
  `OnceLock`, so no test can poison another in the same binary.
- **Why `lock()` and not `try_lock()`.** `parking_lot::Mutex` is not reentrant, but the
  guard is only reachable from the three core markdown tools, none of which hold the
  catalog lock — no librarian tool calls the guard. Noted in the impl so the invariant is
  checkable rather than assumed.
- The refusal message now names *which* reason fired; the augmented form adds "its params
  live in the catalog, and this file is only a rendered snapshot of them". The substring
  `usage/db.rs:282` classifies on is unchanged.

Call sites now pass the resolved path: `read_markdown.rs:503`, `edit_markdown.rs:1213`,
`edit_file/mod.rs` `read_edit_target`.
## Tests added

All in `src/util/librarian_guard.rs` tests. Each was watched fail first.

**Quoting half:**

- `an_id_is_recognised_whatever_yaml_quoting_it_was_serialised_with` — table over bare,
  single-quoted, double-quoted, extra-spaced and unspaced forms. The **bare row was green
  before the fix**, which is exactly how this survived the `47abcb6d` hardening pass: a
  test written with only the unquoted form cannot fail, however many write paths it covers.
  Failed on `single-quoted` after passing `bare`, reproducing the asymmetry.
- `stripping_quotes_does_not_loosen_the_id_rule` — too short, uppercase, non-hex,
  mismatched quotes, unterminated, empty, quote-only. Green by construction; it pins that
  the fix did not buy coverage with false positives.
- `guard_fires_on_a_quoted_id_the_way_it_does_on_a_bare_one` — asserts at
  `guard_not_librarian_managed`, the function all three call sites share.

**Augmentation half:**

- `an_augmented_artifact_is_guarded_even_with_no_frontmatter_id` — the case the text
  predicate provably cannot see. Asserts the no-id precondition explicitly, so the test
  cannot silently start passing for the wrong reason.
- `a_catalogued_but_unaugmented_file_stays_directly_editable` — two rows,
  `skill-frictions.md` and `RELEASE.md`. **Green before the oracle was wired**, on purpose:
  it pins the behaviour the widening must not break, and it is the test that would have
  caught the catalog-membership design had I implemented it.

Gate: 3893 tests (full `cargo test`), clippy `-D warnings`, fmt.
## Workarounds

No longer needed. For the record, the pre-fix habit was: assume no guard, and route
everything under `docs/trackers/` through `artifact(get/update)` regardless of whether a
direct call was refused.

The general form of that habit survives and is worth keeping: **a refusal is informative,
permission is not.** A gate that fires tells you something real about the file; a gate
that stays quiet tells you only that its predicate returned false.
## Resume

N/A — fixed in `29f0c015` and verified live against the running MCP server, 2026-08-16.
Fast-forward promotion (`git rev-list --left-right --count master...experiments` = `0` on
the left), so this SHA is the master SHA; no second one to record.

All four cases checked through the real tool surface, the two negatives mattering as much
as the positives:

```
read_markdown("docs/trackers/artifact-augmentation-followups.md")   # augmented, NO id
-> REFUSED: '…' is a librarian-managed artifact (augmented — its params live in the
   catalog, and this file is only a rendered snapshot of them)

read_markdown("docs/trackers/open-issue-work-queue.md")            # augmented, quoted id
-> REFUSED (same message)

read_markdown("docs/trackers/skill-frictions.md")                  # catalog row, prose
-> ---\nkind: tracker\nstatus: active\ntitle: Skill Frictions Tracker …

read_markdown("docs/RELEASE.md")                                   # catalog row, plain doc
-> # Release & Ship Procedures …
```

The first is the case no amount of string-parsing could reach. The last two are the ones a
catalog-membership guard would have broken.

**Follow-up landed `053238cb`** (F-51 in `docs/trackers/bug-fix-session-log.md`). The
`OnceLock` this fix introduced was first-writer-wins, so in a test binary — where
`from_parts_with_env` builds a server with its own catalog at three call sites — whichever
ran first pinned its catalog and every later install was silently discarded. Now
last-writer-wins, matching `src/heartbeat.rs:47`, this project's only other global mutable
state, whose doc had already chosen those semantics on purpose. No production difference:
one server either way.

One residual, deliberately not closed: an artifact that is neither augmented nor carrying a
stamped id stays directly editable — e.g. a bug file copied from
`docs/issues/_TEMPLATE.md`. Correct as it stands (nothing lives outside the file), and the
class shrinks on its own since `artifact(create)` stamps an id. Do **not** "fix" it by
guarding catalog membership; see *Correction to the numbers above* for the measurement
showing that would refuse `docs/RELEASE.md` and the whole documentation set.
## References

- `src/util/librarian_guard.rs:31-45` — `is_librarian_artifact`
- `docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md` — the coverage measurement this reframes
- `docs/trackers/tool-usage-patterns.md` § T-22 — the session observation that surfaced it

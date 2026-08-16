---
id: a2899c126f1e7771
kind: bug
status: open
title: 'BUG: the librarian guard is keyed on YAML quoting, so 15 of 27 trackers are unprotected'
tags:
- librarian
- guard
- edit_markdown
- read_markdown
- data-loss
closed: null
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

Not implemented. The guard should answer "is this path a managed artifact?" by resolving
the path against the catalog, not by pattern-matching frontmatter. That also fixes the
false-positive half for free — a stale-but-well-shaped id stops mattering.

If a text fast-path must stay (the guard runs on every markdown write, and a catalog hit
is not free), strip surrounding `'` / `"` before the length test. That is a two-line change
and closes the reported hole, but leaves the guard trusting a value the catalog may
disagree with — so treat it as mitigation, not the fix.

Do **not** fix by re-quoting the 15 files: that hides the defect behind a convention no
writer enforces, and the next artifact created by a tool that quotes its YAML reopens it.

## Tests added

None — not fixed. The regression test must be **table-driven over both quoting styles**,
asserting the same verdict for `id: abc…` and `id: 'abc…'`. A test using only the unquoted
form is green against the current code, which is exactly how this survived the
`47abcb6d` hardening pass.

## Workarounds

Assume no guard. Use `artifact(action="get"/"update")` for anything under
`docs/trackers/` regardless of whether a direct read or edit is refused — a refusal is
informative, but permission is not evidence the file is unmanaged.

## Resume

Decide catalog-lookup vs. quote-stripping fast-path (see *Fix*). Then check the other two
guard entry points for the same text-heuristic assumption:
`src/util/librarian_guard.rs` `guard_not_librarian_managed`, and whatever
`read_markdown` calls — the refusal message differs between the read and write paths, so
they may not share one predicate.

Also worth pairing with BL-23 (`6149f4cfeaa6fab9`, "a moved artifact's frontmatter still
asserts its pre-move id"): this bug is how a stale frontmatter id stays invisible, since
the guard validates shape and never value.

## References

- `src/util/librarian_guard.rs:31-45` — `is_librarian_artifact`
- `docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md` — the coverage measurement this reframes
- `docs/trackers/tool-usage-patterns.md` § T-22 — the session observation that surfaced it


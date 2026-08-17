---
id: e55b9263451c6647
kind: bug
status: open
title: 'BUG: audit_doc_refs resolves an `include_str!` argument against the markdown file, failing the CI gate on a correct doc'
tags:
- audit-doc-refs
- false-positive
- ci-gate
- docs
closed: ''
opened: 2026-08-17
owner: marius
related:
- docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md
- docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md
- docs/issues/archive/2026-08-15-audit-doc-refs-classifies-comment-markers-as-paths.md
severity: high
---

## Summary

A markdown code span that quotes Rust — `` `include_str!("./render_template.j2")` `` — is
parsed as a file-path reference and resolved **relative to the markdown file's own
directory**. The path is source-relative: rustc resolves an `include_str!` argument against
the *containing `.rs` file*, and the target does exist at
`src/librarian/tools/legibility_scan/render_template.j2`. So a correct doc produced a
`severity: high`, `verdict: missing` finding — and `--fail-on high`, which is exactly what
CI runs, exited 1.

## Symptom (Effect)

`librarian(action="audit_doc_refs", fail_on="high", emit_tracker=false)` at `a1540c8c`:

```json
{
  "n": null,
  "md_file": "docs/architecture/augmented-artifacts.md",
  "md_line": 232,
  "raw_ref": "./render_template.j2",
  "ref_kind": "file_path",
  "verdict": "missing",
  "severity": "high",
  "severity_reason": "policy_default",
  "status": "open",
  "notes": null
}
```

with `exit_code=1`, and it was the **only** `high` in the run (1 high, 49 med in the shown
window).

The source line it flagged:

```markdown
| `legibility_scan::render_managed_body` | the `.md` file body | **no** — `include_str!("./render_template.j2")`, compiled in, for the `legibility-backlog` tracker only |
```

## Reproduction

```
git checkout a1540c8c
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .
```

This is byte-for-byte the command the `audit-doc-refs` CI job runs
(`.github/workflows/ci.yml:370`).

## Environment

- Branch `experiments`, repo `codescout`, Linux, MCP stdio transport.
- Present from `a1540c8c` (the commit that wrote the table row) onward.
- Scan reported `scan_meta.degraded=true` (`rust: lsp_behind_index`) — **irrelevant to this
  finding**: `ref_kind` is `file_path`, a filesystem check, not an LSP symbol resolution.

## Root cause

**Measured 2026-08-17** (the behaviour): the audit command above → `exit_code=1` with the
finding quoted in *Symptom*. Editing the doc to name the repo-relative path and re-running
→ `exit_code=0`, with `n_refs_resolved` 17477 → 17478 and `n_refs_broken` unchanged at
10384 — the ref moved from broken to resolved, confirming the base directory was the whole
of it.

**Inferred, not yet located to a line:** the parser accepts the `./`-prefixed string inside
the code span as a `file_path` candidate, and the resolver joins it to the *markdown* file's
parent. The specific branch in `src/librarian/tools/audit_doc_refs/parser.rs` and the join in
`src/librarian/tools/audit_doc_refs/resolver.rs` have **not** been read — do not treat this
paragraph as a code citation.

The general shape: a `./`-prefixed path is only doc-relative when the prose *is* the
authority on where it lives. Inside a quoted `<ident>!( … )` macro call, the authority is the
Rust source file, and the doc is a bystander. `include_str!`, `include_bytes!`, `include!`
and `#[path = "…"]` all share it.

## Evidence

### The target does exist, at the Rust base

```
$ find src -name '*.j2' -maxdepth 6
src/librarian/tools/audit_doc_refs/render_template.j2
src/librarian/tools/legibility_scan/render_template.j2
```

Note the audit tool has one of its own — so the basename is **ambiguous**, not missing. The
`Verdict` enum already carries `AmbiguousBasename`, which buckets as `unknown` rather than
`broken` and would not have gated.

### The gate is real

`.github/workflows/ci.yml:345-370` — job `audit-doc-refs`, final step:

```
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .
```

with the comment *"Gate is now `--fail-on high` so any future hi-sev reintroduction fails
the build."* It would have.

## Hypotheses tried

1. **Hypothesis:** the 10,384 `n_refs_broken` indicates broad doc rot, and the `high` is one
   of many.
   **Test:** read `$.findings[*].severity` from the buffered result; read the counter
   definitions at `src/librarian/tools/audit_doc_refs/mod.rs:953,1043`.
   **Verdict:** rejected. `n_refs_found` is `findings.len()` — a "finding" is *every ref
   examined*, resolved ones included — and exactly one finding was `high`.

2. **Hypothesis:** `overflow.total` (46692) is a denominator the tool did not count, the
   same defect as the grep `Showing N of M` family.
   **Test:** read `mod.rs:953` (`let total = findings.len()`).
   **Verdict:** rejected. `total` is the finding count and is correct.

3. **Hypothesis:** the scan is reaching files outside `DEFAULT_AUDIT_GLOBS` (Rust sources,
   `build.rs`, `contrib/pi/codescout-mode.ts` all appear as `md_file`).
   **Test:** read `mod.rs:341-354`.
   **Verdict:** rejected — by design. The default set is `DEFAULT_AUDIT_GLOBS` **chained
   with `DEFAULT_AUDIT_CODE_GLOBS`**; source files are scanned deliberately, to catch bug-file
   citations in code comments that were archived out from under them.

4. **Hypothesis:** the base directory used to resolve the ref is the markdown file's.
   **Test:** rewrite the ref as a repo-relative path, re-run.
   **Verdict:** confirmed. See *Root cause*.

## Fix

**Doc side — landed.** `docs/architecture/augmented-artifacts.md:232` now reads

```markdown
**no** — a compiled-in `include_str!` of `src/librarian/tools/legibility_scan/render_template.j2`, for the `legibility-backlog` tracker only
```

which is both resolvable *and* more useful to a reader — the audit now verifies the claim
instead of being told to look away. Preferred over an `<!-- audit-doc-refs:ignore -->`
marker for that reason.

**Tool side — not started.** Sketch: when a `./`-prefixed ref appears inside a quoted
`<ident>!( … )` form, it is not doc-relative. Either (a) skip it the way
`parser_rejects_glob_patterns` skips globs, or (b) fall through to the existing
basename-resolution path, which for this ref yields `AmbiguousBasename` (two `render_template.j2`
in the tree) and therefore `unknown` — below the gate either way.

Option (b) is the better failure mode: it still resolves a unique basename, so a genuinely
missing include target keeps being caught.

## Tests added

None yet — the tool-side fix is not written. When it is, the regression belongs next to
`parser_rejects_glob_patterns` (`src/librarian/tools/audit_doc_refs/parser.rs`, in the
`#[test]` block around line 984), asserting that a `./`-prefixed macro argument is not
classified as a doc-relative `file_path`.

## Workarounds

Write the repo-relative path in the doc rather than reproducing the macro argument verbatim,
as above. `<!-- audit-doc-refs:ignore -->` also works (precedent:
`docs/manual/src/concepts/librarian-embedded.md:89`) but suppresses a check rather than
satisfying it.

## Resume

Locate the two code paths named as **inferred** in *Root cause* — the `file_path` classifier
branch in `src/librarian/tools/audit_doc_refs/parser.rs` and the base-directory join in
`src/librarian/tools/audit_doc_refs/resolver.rs` — and confirm the mechanism before writing
option (b). The behaviour is measured; the code path is not.

## References

- `.github/workflows/ci.yml:345-370` — the `audit-doc-refs` job.
- `src/librarian/tools/audit_doc_refs/mod.rs:213-220` — `DEFAULT_AUDIT_GLOBS`.
- `src/librarian/tools/audit_doc_refs/mod.rs:341-354` — default glob/exclude selection.
- Sibling false-positive bugs, all fixed: JSON pointers
  (`docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md`),
  `Type/method` slugs
  (`docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`),
  comment markers
  (`docs/issues/archive/2026-08-15-audit-doc-refs-classifies-comment-markers-as-paths.md`).


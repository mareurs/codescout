---
status: open
opened: 2026-08-06
closed:
severity: high
owner: marius
related: []
tags: [librarian, audit_doc_refs, ci, lint-precision]
kind: bug
---

# BUG: audit_doc_refs reads `Type/method` and org/repo slugs as local file paths

> **KNOWN — same root cause as the earlier filing
> `docs/issues/2026-07-28-audit-doc-refs-json-pointer-false-positive.md`
> ("audit_doc_refs flags JSON/config pointers as missing file paths with severity
> high", still open).** That file has priority; this one was opened during a
> merge-readiness pass before its ledger neighbour was consulted — a capture-discipline
> miss, recorded rather than hidden.
>
> Kept as a separate file because it adds three token classes the earlier one does not
> cover (`Type/method` `name_path` syntax, GitHub org/repo slugs, ellipsis-elided
> external paths) plus the evidence that the CI gate is *currently failing* on them.
> Fix them together: it is one extractor and one severity policy. Whoever picks this up
> should read both files and can fold this one in via
> `artifact(action="graft", from_id="56b725405a9c36d1", into_id="21f6d21b3bf82c30")`.
## Summary

`audit_doc_refs` classifies any `A/B` token in prose as a `file_path` ref. That
makes codescout's own `name_path` symbol syntax (`SensitiveString/fmt`), GitHub
org/repo slugs (`Kotlin/kotlin-lsp`), and elided external paths
(`…/rocks/v492/LOCK`) all resolve as missing local files at severity `high`.
Since CI runs the audit at `--fail-on high` with no `continue-on-error`, these
false positives fail the build.

Every one of the 18 high-severity findings currently reported is a false
positive. There is no real doc drift among them.

## Symptom (Effect)

```
$ cargo run --bin codescout -- audit-doc-refs --no-emit-tracker --fail-on high --json --project .
=== CLI EXIT: 1 ===
```

The 18 findings, all `verdict: "missing"`, `severity: "high"`,
`severity_reason: "policy_default"`:

| `raw_ref` | `ref_kind` | What it actually is |
|---|---|---|
| `SensitiveString/fmt` ×4, `Type/method` ×3, `LspClient/hover` | `file_path` | codescout `name_path` symbol syntax |
| `Kotlin/kotlin-lsp`, `JetBrains/utils/kotlin-lsp` | `file_path` | GitHub org/repo slugs |
| `RELEASES.md` ×2 | `file_path` | a file in the *upstream* kotlin-lsp repo |
| `rocks/v492/LOCK` ×2, `…/rocks/v492/LOCK` ×2 | `file_path` | an elided external RocksDB lock path |
| `process.rs:66-135` ×2 | `file_line` | bare basename plus line range |

Spread across three ADRs:

```
8  docs/adrs/2026-06-13-drop-name-collision-defect.md
5  docs/adrs/2026-06-11-mux-single-owner-invariant.md
5  docs/adrs/2026-06-11-kotlin-lsp-upgrade-decoupled-from-sharing.md
```

A scoped run over the human-facing surfaces surfaces the same class from
docs that teach by example — `audit-doc-refs.md` cites `src/foo.py` and
`scripts/eval.py:807` to illustrate the ref kinds the tool detects, and
`artifact-move.md` cites `docs/trackers/my-tracker.md` as an example argument.
Those are correct prose that the lint cannot distinguish from drift.


### A fourth class, at `med` rather than `high`

mdBook **relative links** are reported `verdict: "ambiguous_basename"`,
`severity: "med"`. Example: `docs/manual/src/agents/overview.md:21` links
`[Claude Code](claude-code.md)`, and the audit reports *"basename matches 2
files: docs/agents/claude-code.md, docs/manual/src/agents/claude-code.md"*.

The link is **correct** — mdBook resolves it relative to the containing file, and
lengthening the path to disambiguate would break the book. The audit resolves
basenames globally, which is the wrong resolution rule for a relative link inside
a known book root.

This class does not gate CI (`--fail-on high`), so it is noise rather than a
blocker. It belongs in the same fix: relative links inside `docs/manual/src/`
should resolve against their own directory first and only report ambiguity if
*that* fails. Do not "fix" these by rewriting the links.
## Reproduction

At `7938d68b` on `experiments`:

```bash
cargo run --bin codescout -- audit-doc-refs --no-emit-tracker --fail-on high --json --project .
echo $?   # 1
```

## Environment

Linux, stable 1.95.0, branch `experiments`. Same command and flags as
`.github/workflows/ci.yml` job `audit-doc-refs`.

## Root cause

The ref extractor treats a bare `A/B` token as a relative file path. It has no
discriminator for:

- **`Type/method`** — which is codescout's documented `name_path` form, accepted
  by `symbols(name_path=…)` and `edit_code(symbol=…)`. The audit already has a
  `file_symbol` ref kind (`path.rs:symbol`); a bare `Type/method` with no file
  part is being routed to `file_path` instead.
- **`org/repo`** — a slug, usually adjacent to a `github.com` URL in the same
  sentence.
- **A leading ellipsis** (`…/` or `.../`) — an explicit author signal that the
  path is elided and not resolvable.

The severity assignment compounds it: `severity_reason: "policy_default"` gives
`missing` the `high` band regardless of how confident the classification was, so
a guess and a certainty are indistinguishable to the gate.

## Evidence

### Local run at 7938d68b

Exit 1, 18 high-severity findings, listed under *Symptom*. Full corpus scan for
scale: 874 files, 47,516 refs found, 10,404 broken — the overwhelming majority
in `docs/superpowers/plans/`, `docs/archive/` and `docs/trackers/archive/`,
which are dated records citing paths that legitimately moved.

### CI has been failing on this job

`gh run view 30852803569 --json jobs` shows `failure  Audit Doc Refs`, with
`##[error]Process completed with exit code 1`. The job has no
`continue-on-error`, so `docs/RELEASE-TODO.md`'s description of it as
"informational" contradicts `docs/RELEASE-TODO.md`'s own other entry calling it
a gate at `--fail-on high`. The workflow is authoritative: it is a gate.

## Hypotheses tried

1. **Hypothesis** — the high-severity findings are real doc drift from the
   387-commit cohort.
   **Test** — read all 18 `raw_ref` values and classified each by hand against
   the source tree and the ADR prose around it.
   **Verdict** — rejected. All 18 are false positives; the ADR prose is correct
   in every case.
   **Evidence link** — *Local run at 7938d68b*.

2. **Hypothesis** — CI passes where local fails because CI lacks LSP servers, so
   symbol refs degrade to `unknown` instead of `missing`.
   **Verdict** — rejected. These are `file_path` / `file_line` kinds, resolved
   against the filesystem with no LSP involvement, and CI does report the job as
   failed.

## Fix

Not implemented. Two independent changes, both worth doing:

1. **Extractor precision.** Do not classify a token as `file_path` when it
   (a) matches `^[A-Z][A-Za-z0-9_]*/[a-z_][A-Za-z0-9_]*$` (the `name_path`
   shape), (b) begins with `…/` or `.../`, or (c) appears inside or adjacent to
   a `github.com/<org>/<repo>` URL in the same line. Route (a) to a
   symbol-index lookup instead.
2. **Severity honesty.** A `missing` verdict on a ref whose *kind* was inferred
   rather than syntactically unambiguous (no `/` file extension, no `./`
   prefix) should not land in the `high` band. `high` should mean "this is
   definitely a local path and it is definitely gone".

Do NOT fix this by rewriting the ADR prose. The three ADRs are accurate; editing
them to appease the lint would trade correct documentation for a green check.

## Tests added

N/A — not fixed. When fixed, the natural regression corpus is the 18 refs in
*Symptom*: each should classify as non-`file_path`, or as `file_path` at a
severity below `high`.

## Workarounds

Run the audit scoped to surfaces that do not teach by example:

```text
librarian(action="audit_doc_refs", paths=["README.md", "CLAUDE.md", "docs/manual/src/**/*.md"])
```

For CI specifically, the choices are to fix the extractor, to drop the gate to
`--fail-on never` until it is fixed (and say so in the workflow comment, which
currently claims all high-severity findings are reconciled), or to add
`continue-on-error: true` to match the "informational" description in
`docs/RELEASE-TODO.md`. Leaving a hard gate that fails on false positives is the
one option that has no upside — it trains everyone to ignore a red check.

## Resume

Start with the `name_path` case, which is 8 of the 18 and the least ambiguous
fix: in the extractor, before emitting a `file_path` ref, test the token against
the `name_path` shape and route it to the symbol index. Re-run
`cargo run --bin codescout -- audit-doc-refs --no-emit-tracker --fail-on high --json --project .`
and confirm the count drops from 18 to 10. Then handle the `…/` prefix (4) and
the `github.com` adjacency (4).

## References

- `src/librarian/tools/audit_doc_refs/` — extractor and severity policy
- `.github/workflows/ci.yml` — job `audit-doc-refs`, line 170
- `docs/RELEASE-TODO.md` — the two contradicting descriptions of this gate
- `docs/manual/src/concepts/audit-doc-refs.md` — the page whose own examples trip it
- CI run: https://github.com/mareurs/codescout/actions/runs/30852803569

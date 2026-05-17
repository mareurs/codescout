---
status: fixed
opened: 2026-05-17
closed: 2026-05-17
severity: medium
owner: marius
related: []
tags: ["librarian", "audit_doc_refs", "doc-refs", "false-positives", "basename-resolution"]
---

# BUG: `audit_doc_refs` flags bare-basename file references as `missing` with severity `high`

## Summary

`librarian(audit_doc_refs)` reported file references mentioned by bare basename
(e.g., `` `docling_reader.py` `` written without the `src/mrv/readers/` prefix)
as `verdict: missing` / `severity: high`. The audit's path resolver did
literal repo-root filesystem lookup with no basename fallback. Result on
MRV-poc: 4,120 of 11,099 refs (37%) reported broken — most were conversational
basename mentions of files that exist under standard subdirs.

**Fixed 2026-05-17** via basename-fallback resolver. When literal path lookup
fails AND `raw_ref` has no `/`, the resolver consults a basename index
built once per audit run (via `ignore::WalkBuilder`). Three new verdict
tiers: `resolved_basename` (1 match, severity Low), `ambiguous_basename`
(>1 match, severity Med), `missing` (0 matches, severity High — unchanged).
## Symptom (Effect)

Running `librarian(action="audit_doc_refs")` on MRV-poc produces a findings
list where many `high`-severity broken refs point at files that exist:

```
{
  "md_file": "docs/adr/0006-fixture-label-resolver.md",
  "md_line": 99,
  "raw_ref": "docling_reader.py",
  "ref_kind": "file_path",
  "verdict": "missing",
  "severity": "high",
  "severity_reason": "policy_default"
}
```

…but the file exists:

```
$ ls src/mrv/readers/docling_reader.py
src/mrv/readers/docling_reader.py
```

Same pattern for:
- `_validate_fixture.py` → exists at `scripts/_validate_fixture.py`
- `run_trial.py` → exists at `scripts/run_trial.py`
- `_repin_v3_fixtures.py`, `_repin_xlsx_gold.py`, `_f9_finalize_fixture.py`,
  `xlsx_openpyxl_reader.py` → all exist under `scripts/` or `src/mrv/readers/`

Aggregate impact (MRV-poc, 2026-05-17 run):

| Metric | Count | % |
|---|---|---|
| Refs found | 11,099 | 100 % |
| Resolved | 4,151 | 37 % |
| Broken (`missing` / `file_missing`) | 4,120 | 37 % |
| Unknown | 2,800 | 25 % |

The 37 % broken figure is the headline — but most of those refs are not
actually broken paths; they are basename mentions in prose.

## Reproduction

```bash
# In any project with markdown that mentions Python files by basename:
mcp call codescout librarian '{"action":"audit_doc_refs","emit_tracker":true}'
# → inspect findings; high-severity hits include basename-only refs that
#   resolve via fuzzy filesystem search
```

Specifically on MRV-poc, commit `808fe4b` (branch `dev`):

```bash
# audit returns ~22 high-severity hits in the visible top-50 findings,
# of which the ADR-0006 cluster (docling_reader.py, _validate_fixture.py,
# run_trial.py, etc.) are all bare-basename false positives.
```

## Environment

- Date observed: 2026-05-17
- Tool: `mcp__codescout__librarian(action="audit_doc_refs")`
- Component: librarian audit ref resolver (path: TBD — needs source check)
- Project: MRV-poc (266 markdown files, 11099 refs)
- Codescout: current build at code-explorer HEAD as of 2026-05-17

## Root cause

Unknown — under investigation. Best lead: the audit's `file_path` resolver
likely does a literal `Path(repo_root).join(raw_ref).exists()` check. When
`raw_ref` is a bare basename like `docling_reader.py`, this resolves to
`<repo_root>/docling_reader.py`, which doesn't exist, so the verdict flips to
`missing`. No fallback step searches the indexed file table for filename
matches.

Compounding factor: `severity_reason: policy_default` is the same value for
every high-severity hit in the visible sample (22/22). The severity policy
appears to assign `high` purely on `ref_kind: file_path` + `verdict: missing`
— it doesn't downgrade based on **specificity** of the ref. A bare
`manifest.json` mention and a precise `docs/adr/0006-fixture-label-resolver.md`
mention get equal weight; only one is actionable.

## Evidence

### MRV-poc audit output — top-50 findings sample

22 of 50 sampled findings have `severity: high`. Spot-checked 7:

| md_file:line | raw_ref | verdict | Exists at |
|---|---|---|---|
| `docs/adr/0006-fixture-label-resolver.md:99` | `docling_reader.py` | missing | `src/mrv/readers/docling_reader.py` ✓ |
| `docs/adr/0006-fixture-label-resolver.md:99` | `xlsx_openpyxl_reader.py` | missing | — (genuinely absent, likely renamed) |
| `docs/adr/0006-fixture-label-resolver.md:107` | `_f9_finalize_fixture.py` | missing | `scripts/_f9_finalize_fixture.py` ✓ |
| `docs/adr/0006-fixture-label-resolver.md:108` | `_validate_fixture.py` | missing | `scripts/_validate_fixture.py` ✓ |
| `docs/adr/0006-fixture-label-resolver.md:153` | `run_trial.py` | missing | `scripts/run_trial.py` ✓ |
| `docs/adr/0002-gemini-vertex-ai-sole-llm-backend.md:41` | `docs/trackers/vertex-eu-multiregion-sdk-quirk.md` | missing | — (genuinely absent — finding lives in codescout memory only) |
| `docs/adr/0005-section-filter-disabled.md:9` | `manifest.json` | missing | — (genuinely absent; now `data/manifest.json`) |

5 of 7 sampled `high` hits are false positives caused by basename-only
references. 2 are real drift.

### Tracker artifact

`librarian(audit_doc_refs)` emits `docs/trackers/doc-ref-audit.md`
(artifact `66bee6230b115240`) on MRV-poc, but the tracker body is just
the frontmatter + a comment line — full findings live in the JSON
return / augmentation params, not the rendered body.

## Hypotheses tried

1. **Hypothesis:** The codescout file/symbol index is stale and the audit
   reflects pre-reindex state.
   **Test:** Ran `workspace(post_compact=true)`; codescout reports LSP
   caches flushed. Re-ran spot-check `ls` on a sample of "missing" refs.
   **Verdict:** rejected.
   **Evidence link:** [Evidence — MRV-poc audit output sample](#mrv-poc-audit-output--top-50-findings-sample) — files
   still exist on disk; audit still reports them missing.

2. **Hypothesis:** These are real drift — files were renamed/moved and the
   docs weren't updated.
   **Test:** `ls` on each "missing" basename ref under `src/` and `scripts/`.
   **Verdict:** rejected for 5/7 sampled. (2/7 are real drift — see
   above table.)
   **Evidence link:** [Evidence — MRV-poc audit output sample](#mrv-poc-audit-output--top-50-findings-sample).

3. **Hypothesis:** Severity policy is calibrated on path-specificity but the
   audit isn't surfacing it.
   **Test:** Inspected `severity_reason` field across all 22 high-severity
   hits in the visible sample.
   **Verdict:** rejected — every hit has `severity_reason: "policy_default"`.
   Policy is uniform; specificity is not consulted.
   **Evidence link:** see Evidence section.

## Fix

Shipped 2026-05-17 as option 1 (basename-fallback) from the design notes
below. Implementation:

- **`build_basename_index`** in `src/librarian/tools/audit_doc_refs/mod.rs` —
  walks `repo_root` with `ignore::WalkBuilder` once per audit run, builds
  `HashMap<String, Vec<PathBuf>>` (basename → relative paths). Soft cap of
  50000 files for monorepos.
- **`ResolveCtx.basename_index`** added to `resolver.rs` — owned map, passed
  by reference to each `resolve_ref` call.
- **`try_basename_fallback`** helper in `resolver.rs` — consulted by both
  `resolve_file_path` and `resolve_link` (fs-scheme branch) when literal
  lookup misses and raw_ref has no `/`.
- **Two new `Verdict` variants**: `ResolvedBasename`, `AmbiguousBasename`.
  `severity::default_severity` updated; `merger.rs` `is_resolved_verdict`
  helper treats `ResolvedBasename` the same as `Resolved` for tracker
  bookkeeping (auto-resolve/regress transitions).
## Tests added

- `resolver_resolves_by_basename_when_unique` — single-match path.
- `resolver_ambiguous_when_basename_matches_multiple_files` — multi-match path.
- `resolver_still_missing_when_basename_not_in_index` — 0-match still Missing/High.
- `resolver_skips_basename_fallback_when_ref_contains_slash` — explicit-path
  refs are NOT second-guessed by the fallback.

42 audit_doc_refs tests pass (38 pre-existing + 4 new).
## Workarounds

Post-filter findings before treating the audit as a punch list:

```python
# Pseudocode — apply to findings JSON
actionable = [
    f for f in findings
    if f["verdict"] == "missing"
    and ("/" in f["raw_ref"] or f["raw_ref"].startswith(("docs/", "src/", "scripts/", "benchmarks/")))
]
```

On MRV-poc, this cuts the 4,120 "missing" findings to a much smaller
actionable set (estimated <500). Until the resolver or severity policy is
fixed, this manual triage is the only honest way to consume the audit.

## Resume

Already shipped. Next steps in a follow-up:

1. Re-run the audit on MRV-poc and confirm the false-positive rate drops
   (target: <5% basename-related noise).
2. Pick up #49 (Bug-file Resume path linter) — the broad scope is now
   viable because audit_doc_refs is no longer drowning in basename noise.
3. If `AmbiguousBasename` findings turn out to be useful in practice,
   consider extending the notes payload to include a heuristic "likely
   intended" guess (e.g. prefer paths near the md_file's directory).
## References

- MRV-poc audit run: 2026-05-17, branch `dev`, commit `808fe4b`
- Tracker: `docs/trackers/doc-ref-audit.md` (artifact `66bee6230b115240`)
- Findings JSON: 4,120 broken refs, 22 high-severity in top-50 sample
- Related session memory: MRV-poc `gotchas` (codescout memory)
- Template: `~/work/claude/code-explorer/docs/issues/_TEMPLATE.md`

---
status: fixed
opened: 2026-08-06
closed: 2026-08-06
severity: high
owner: marius
related: ["56b725405a9c36d1", "21f6d21b3bf82c30"]
tags: [librarian, audit_doc_refs, ci, progressive-disclosure, silent-cap]
kind: bug
---

# BUG: audit_doc_refs exits 1 but its 50-finding cap hides every finding that caused it

## Summary

`audit_doc_refs` truncates its `findings` array to the first 50 **in scan order**
while computing the exit code over **all** findings. Since the overwhelming
majority of refs resolve, the 50 shown are almost always `resolved`/`low` — so a
`--fail-on high` run reports failure with no visible cause. This also silently
corrupted the measurement in a sibling bug: the "18 high-severity findings"
recorded in
`docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` was the
count *within the truncated window*, not the real population.

## Symptom (Effect)

```
$ cargo run --bin codescout -- audit-doc-refs --no-emit-tracker --fail-on high --json --project .
=== CLI EXIT: 1 ===

$ grep -c '"severity": "high"' out.json
0
```

Exit 1 means at least one unresolved `high` finding exists, yet zero appear in the
output. The envelope reports `"shown": 50, "total": 46572` — and `total` counts
every ref scanned, resolved ones included, so it reads as "50 of 46572 problems"
when it is really "50 of 46572 refs, of which 8875 are broken".

## Reproduction

At `184dbced` on `experiments`, before the fix:

```bash
cargo run --bin codescout -- audit-doc-refs --no-emit-tracker --fail-on high --json --project . > out.json
echo $?                                  # 1
grep -c '"severity": "high"' out.json    # 0
```

## Environment

Linux, Rust 1.95.0, codescout 0.15.0, branch `experiments`, default features.

## Root cause

`build_response` (`src/librarian/tools/audit_doc_refs/mod.rs`) truncated without
ordering:

```rust
let cap = 50;
let total = findings.len();
let shown_findings: Vec<_> = findings.iter().take(cap).map(finding_to_json).collect();
```

`findings` arrives in scan order — file by file, ref by ref within each file — and
carries every ref including `Verdict::Resolved`. The exit code, computed a few
lines below, filters to unresolved findings across the **whole** vector:

```rust
"high" => findings.iter().any(|f| counts(f) == Some(Severity::High)) as i32,
```

So the two consumers disagree about which findings matter: the gate looks at all
of them by severity, the output shows an unfiltered, unordered prefix. Nothing
guarantees overlap, and with ~24 % of refs resolving and highs being rare, in
practice there is none.

The tracker path (`upsert_tracker`) does receive the full findings vector, so the
data was never lost — but `--no-emit-tracker` is exactly what CI passes, and the
CLI's tracker emission returned `tracker_id: null` in local runs anyway, leaving
the JSON output as the only surface.

## Evidence

### The two consumers, side by side

- Truncation: `findings.iter().take(50)` — scan order, includes `resolved`.
- Gate: `findings.iter().any(|f| counts(f) == Some(Severity::High))` — all
  findings, unresolved only.

### Downstream measurement error

`docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` tabulates
"The 18 findings, all `verdict: "missing"`, `severity: "high"`" across three ADRs.
After fixing every one of those 18 classes (verified: each named `raw_ref` now
resolves or reports `med`/`inferred_path`), the gate still exited 1. A
per-subdirectory bisect found high findings surviving in **11 of 16** `docs/`
subdirectories:

```
adrs=1 architecture=1 archive=0 conventions=0 evals=1 issues=0 lessons=1
manual=1 plans=1 research=1 reviews=1 spikes=0 superpowers=1 templates=0
trackers=1 usage-reports=1
```

The 18 was never the population; it was the sample the cap happened to expose.

## Hypotheses tried

1. **Hypothesis:** the surviving highs were newly introduced by the extractor
   changes in the sibling bugs.
   **Test:** the changes only remove candidates and lower severities; re-ran the
   pre-change baseline and grepped the shown window.
   **Verdict:** rejected — the baseline's shown window also contained exactly 18
   highs while `n_refs_broken` was 10487, i.e. the window was never
   representative.

2. **Hypothesis:** the tracker would expose the full list, making the cap a
   non-issue.
   **Test:** ran without `--no-emit-tracker`.
   **Verdict:** rejected — `tracker_id`/`tracker_path` came back `null` and no
   `docs/trackers/audit-issues.md` was created, so the JSON output is the only
   available surface. (That emission gap is a separate defect, not filed here
   because it was not reproduced deliberately; see *Resume*.)

## Fix

Rank before truncating, so the shown window always leads with whatever drives the
exit code. Presentation-only — the exit code still considers every finding.

`src/librarian/tools/audit_doc_refs/mod.rs`, `build_response`:

```rust
let mut ranked: Vec<&Finding> = findings.iter().collect();
ranked.sort_by_key(|f| {
    let resolved = matches!(f.resolution.verdict, Verdict::Resolved | Verdict::External);
    let sev = match f.resolution.severity {
        Severity::High => 0u8,
        Severity::Med => 1,
        Severity::Low => 2,
    };
    (resolved, sev)
});
let shown_findings: Vec<_> = ranked.iter().take(cap).map(|f| finding_to_json(f)).collect();
```

`sort_by_key` is stable, so scan order is preserved within a rank. The overflow
`hint` now states that the shown findings are ordered most-severe-first, so a
reader knows the absence of a high finding in the window is meaningful.

Experiments-branch SHA: recorded on commit (this change ships with the
extractor-precision work). Master-side SHA still needs recording after
cherry-pick per CLAUDE.md § "After cherry-pick".

## Tests added

Covered indirectly by the existing `outputguard_caps_findings_inline` (asserts the
cap still applies) plus the whole `fail_on_*` family (asserts the exit code is
unchanged by the reordering) — all 89 `audit_doc_refs` tests green.

A dedicated ordering test is **owed**: assert that a findings vector whose first
50 entries are all `Resolved` and whose 51st is `Missing`/`High` puts the high one
in `shown_findings`. Not added yet because `build_response` takes `&[Finding]` and
`Finding` construction in tests currently goes through `mk_finding`, which the
existing tests use only for `fail_on` assertions — wiring a 51-element fixture is
mechanical but was deferred to keep this change reviewable. See *Resume*.

## Workarounds

Bisect with `--paths`, one directory at a time, until each subset is small enough
that its findings fit inside the cap:

```bash
for d in docs/*/; do printf '%-24s ' "$d"; \
  cargo run -q --bin codescout -- audit-doc-refs --no-emit-tracker \
    --fail-on high --paths "$d**/*.md" --project . >/dev/null 2>&1; echo "exit=$?"; done
```

## Resume

Two follow-ups, both small:

1. Add the deferred ordering regression test in
   `src/librarian/tools/audit_doc_refs/mod.rs` tests: build 51 findings, 50
   `Resolved`/`Low` then one `Missing`/`High`, call `build_response(..., "high")`,
   and assert the high finding appears in `result["findings"]`. Anchor with
   `cargo test --lib audit_doc_refs::tests`.
2. Investigate why the CLI returned `tracker_id: null` without
   `--no-emit-tracker`. Start at `upsert_tracker` /
   `ensure_default_tracker` in `src/librarian/tools/audit_doc_refs/mod.rs` and
   check whether the CLI path supplies a `ToolContext` with a current project.
   File separately if confirmed.

## References

- `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` —
  the bug whose measurement this defect corrupted.
- `docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md` —
  same subsystem, extractor polarity.
- `docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md` —
  the omnibus silent-cap family this belongs to.
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — "Treating the summary as authoritative"
  anti-pattern. (There is no docs/PROGRESSIVE_DISCLOSURE.md; five other docs still
  cite that nonexistent name — see the drift backlog.)
- `docs/issues/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` — the
  remaining population, including the wrong-guide-name citations above.

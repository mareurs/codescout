---
id: '851fa9e1096ff1fa'
kind: bug
status: fixed
title: 'BUG: audit_doc_refs'' gitignore cap consults .gitignore but not the index, dropping refs under tracked-but-ignored dirs below the CI gate'
tags:
- audit-doc-refs
- librarian
- gitignore
- ci-gate
closed: 2026-08-06
opened: 2026-08-06
owner: marius
related: []
severity: medium
---

# BUG: audit_doc_refs' gitignore cap consults `.gitignore` but not the index

## Summary

`is_gitignored` decides whether a missing ref points at "a location git will never track" by
asking the `.gitignore` matcher alone. Gitignore rules do not apply to files already in the
index, so a **tracked file under an ignored directory** is misclassified as untracked. In this
repo that is all of `.github/` — including both CI workflow files. The effect is a one-level
severity drop, which lands such findings at `med`, i.e. *below* the `fail_on: high` CI gate.

## Symptom (Effect)

Two missing refs, identical in every respect except directory, get different severities:

```json
{ "raw_ref": "src/nope-does-not-exist.rs",
  "verdict": "missing", "severity": "high", "severity_reason": "policy_default" }

{ "raw_ref": ".github/workflows/nope-does-not-exist.yml",
  "verdict": "missing", "severity": "med", "severity_reason": "gitignored_path" }
```

## Reproduction

Executed 2026-08-06 at `a53f1760`.

1. Create a probe doc containing two inline code spans — `src/nope-does-not-exist.rs` and
   `.github/workflows/nope-does-not-exist.yml`. Place it where no severity drop applies
   (not under `docs/issues/`, `docs/trackers/`, or any `archive/`).
2. `librarian(action="audit_doc_refs", paths=["<probe>"], emit_tracker=false)`
3. The control comes back `high`; the `.github` ref comes back `med` with
   `severity_reason: "gitignored_path"`.

## Environment

Linux, branch `experiments`, MCP stdio transport, project `codescout`.

## Root cause

`src/librarian/tools/audit_doc_refs/resolver.rs:544` — `is_gitignored` documents itself as
"Whether `raw_ref` names a location git will never track", but its body only asks
`Gitignore::matched_path_or_any_parents`. Those two questions are not the same: **gitignore
rules have no effect on paths already in the index.**

This repo is a live counterexample. `.gitignore` line 1 is `/.github/`, while five files under
it are tracked:

```
.github/ISSUE_TEMPLATE/bug_report.md
.github/ISSUE_TEMPLATE/feature_request.md
.github/pull_request_template.md
.github/workflows/ci.yml
.github/workflows/manual.yml
```

So the directory rule matches, `matched_path_or_any_parents` reports ignore, and the cap fires
on paths that git tracks and CI executes.

`git check-ignore -v .github/workflows/ci.yml` prints nothing, because check-ignore skips
indexed paths by default — git itself distinguishes the two questions. The matcher does not.

## Evidence

### Probe run (quoted above)

Control `high`/`policy_default` vs `.github` `med`/`gitignored_path`, same verdict `missing`,
same ref_kind `file_path`, same file, adjacent lines.

### The tracked-file list

`git ls-files .github/` output, quoted under Root cause.

## Hypotheses tried

1. **Hypothesis:** `.github` is not really ignored and the `git add` warning was spurious.
   **Test:** `grep -n github .gitignore`. **Verdict:** rejected — line 1 is `/.github/`.
2. **Hypothesis:** the cap is harmless because such paths resolve anyway.
   **Test:** the probe above, using a deliberately missing path.
   **Verdict:** rejected — for a *missing* path the cap fires and drops severity one level.
3. **Hypothesis:** the drop lands at `low`, keeping it visibly separate from real findings.
   **Test:** read `severity` in the probe output. **Verdict:** rejected — it lands at `med`,
   which is where the ~8.4k pre-existing broken refs already sit, so it is indistinguishable
   from the accepted backlog.

## Fix

**Implemented** — `598624be` (**experiments**; not yet on `master`).

The matcher is now paired with an index-derived directory set: `TrackedIgnore` in
`src/librarian/tools/audit_doc_refs/resolver.rs`, built by `build_tracked_dirs` in
`src/librarian/tools/audit_doc_refs/mod.rs` from `git2`'s index — a library binding, not a
`git ls-files` subprocess.

`is_gitignored` now decides in three steps:

1. A rule naming the path *itself* is decisive. `/target`, `/.mcp.json` and
   `.codescout/private-memories/` are refused on their own account.
2. If no rule reaches the path even through parents, it is not ignored.
3. Only when a rule reaches it *solely* through a parent does the tracked set decide — a
   parent holding tracked files means git demonstrably tracks things there, so a missing
   ref is a real defect rather than normal absence.

Two ordering details are load-bearing:

- **Own-path before parent.** Every root-level ignored file has the repo root as its
  parent, and the root always contains tracked files, so checking the parent first would
  stop capping `/.mcp.json`.
- **Immediate parent, never any ancestor.** `""` (the root) is in the set whenever
  anything is tracked at all, so an ancestor walk would degenerate into never capping
  anything.

Bundled into the existing `gitignore` field as `Option<TrackedIgnore>` rather than added as
a second field, so the fourteen tests that opt out with `gitignore: None` stayed untouched.

Also narrowed the rule that produced this repo's live instance: `6ca02767` replaces
`/.github/` with `/.github/copilot-instructions.md`, the single generated file `4dac3a3c`
was actually trying to suppress. That removes the occurrence; the code fix removes the
class.
## Tests added

`resolver_still_gates_a_missing_path_under_a_tracked_but_ignored_directory` in
`src/librarian/tools/audit_doc_refs/resolver.rs`. Three refs cover the whole decision
tree: parent rule + tracked parent (must stay `high`), own-path rule + tracked parent (must
cap), parent rule + untracked parent (must cap).

**Verified by mutation, not by passing.** Inverting the single tracked-set check kills two
tests in opposite directions — this one at `med` where `high` is required, and the
pre-existing `resolver_caps_missing_paths_that_gitignore_declares_generated_or_local` at
`high` where `med` is required. A test that passes with *and* without the fix proves
nothing.

**Live end-to-end verification** through the real `codescout audit-doc-refs` binary, run
with the broad `/.github/` rule temporarily restored so the code fix was isolated from the
`.gitignore` narrowing:

| ref | before | after |
|---|---|---|
| `src/nope-does-not-exist.rs` (control) | `high` / `policy_default` | `high` / `policy_default` |
| `.github/workflows/nope-does-not-exist.yml` | `med` / `gitignored_path` | **`high` / `policy_default`** |
| `.codescout/private-memories/nope.md` | `med` / `gitignored_path` | `med` / `gitignored_path` |

Gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, 3505 tests / 0
failed / 44 ignored.
## Workarounds

Filter findings by reason rather than trusting the band:

```
severity_reason == "gitignored_path"  →  review by hand
```

Raising `fail_on` to `med` is not viable — roughly 8.4k pre-existing broken refs already sit
at `med` by design.

## Resume

Fixed and verified, including end-to-end through a real binary rather than the test suite
alone. **One bookkeeping item:** `598624be` and `6ca02767` are **experiments** SHAs. After
the cohort reaches `master`, record the master-side SHA in the Fix section above. An
experiments SHA orphans on rebase, and nothing re-reads `docs/issues/archive/` to repair
it.
## References

- `src/librarian/tools/audit_doc_refs/resolver.rs:544` — `is_gitignored`
- `src/librarian/tools/audit_doc_refs/severity.rs` — `cap_gitignored_path`
- `docs/trackers/release-promotion-session-log.md` — the session that introduced the cap (W-7,
  fresh-clone verification) and this session's F-7 / F-8

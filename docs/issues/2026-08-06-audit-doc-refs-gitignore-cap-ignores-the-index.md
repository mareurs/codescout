---
id: '851fa9e1096ff1fa'
kind: bug
status: open
title: 'BUG: audit_doc_refs'' gitignore cap consults .gitignore but not the index, dropping refs under tracked-but-ignored dirs below the CI gate'
tags:
- audit-doc-refs
- librarian
- gitignore
- ci-gate
closed: ''
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

Not yet implemented. Plan: load the tracked-path set once per scan (`git ls-files`) into
`ResolveCtx` beside `gitignore`, and short-circuit `is_gitignored` to `false` when the ref's
**parent directory contains at least one tracked file**. That keeps the cap doing its intended
job — `.worktrees/`, `.git/`, `.codescout/private-memories/` have no tracked contents, so they
still cap — while `.github/workflows/` stops being treated as untracked.

Prefix-set membership on the parent is the right granularity: per-file index lookup would
still miss the case this bug is about, since the whole point is that the cited file does *not*
exist.

## Tests added

None yet — status is `open`, not fixed. The regression test to add alongside the fix is a
two-ref fixture asserting the control stays `high` **and** that a missing path under a
tracked-but-ignored directory also stays `high`, with an over-match guard proving a genuinely
untracked ignored directory still caps.

## Workarounds

Filter findings by reason rather than trusting the band:

```
severity_reason == "gitignored_path"  →  review by hand
```

Raising `fail_on` to `med` is not viable — roughly 8.4k pre-existing broken refs already sit
at `med` by design.

## Resume

Add `tracked_prefixes: HashSet<PathBuf>` to `ResolveCtx` in
`src/librarian/tools/audit_doc_refs/resolver.rs`, populated in `build_gitignore`'s sibling
constructor in `src/librarian/tools/audit_doc_refs/mod.rs` from `git ls-files`; gate
`is_gitignored` on parent-prefix membership before consulting the matcher. Then re-run the
probe from Reproduction and assert both refs come back `high`.

## References

- `src/librarian/tools/audit_doc_refs/resolver.rs:544` — `is_gitignored`
- `src/librarian/tools/audit_doc_refs/severity.rs` — `cap_gitignored_path`
- `docs/trackers/release-promotion-session-log.md` — the session that introduced the cap (W-7,
  fresh-clone verification) and this session's F-7 / F-8


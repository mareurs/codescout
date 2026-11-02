---
id: '9db8908e81bc4b26'
kind: bug
status: open
title: 'BUG: `audit-doc-refs --fail-on high` over the whole repository exits 1 on 11 findings: five artifact ids cited in six documents since 2026-09 resolve to no artifact'
owners:
- marius
tags:
- cluster/unclassified
- librarian
- doc-ref-audit
opened: 2026-11-02
owner: marius
severity: low
unverified: Cause not established. The ids were checked on this machine's catalog only; the previous full-repository run was not compared.
---

# BUG: `audit-doc-refs --fail-on high` over the whole repository exits 1 on 11 findings: five artifact ids cited in six documents since 2026-09 resolve to no artifact

## Summary

On 2026-11-02, `codescout audit-doc-refs --fail-on high` (no `--paths`) scanned 3,050 files and exited 1. It reported 11 `high` findings, all with verdict `artifact_missing`. They name five ids in six files. `doc(action="get")` answers "unknown artifact id" for each of the five on this machine.

## Class fit

No class on the roster was walked. `cluster/unclassified` is the sanctioned escape.

## Symptom (Effect)

A gate that runs the audit over the whole repository fails, and the failing findings are in files that nobody touched recently. Runs scoped with `--paths` to the files a commit touches exit 0, so a per-commit check does not see them.

| Id | Cited at |
|---|---|
| `1fb2cb7b4614e8b5` | `docs/evals/phase1b-local-classifier-preregistration.md:210` (as "bug `1fb2…`"), `docs/trackers/review-catches.md:89` |
| `626ce2d3fa74dd7c`, `fb7592ff7c344255` | `docs/superpowers/plans/2026-09-09-doctor-per-project-isolation.md:1219` |
| `82973a1e83aa069f` | `docs/trackers/2026-09-21-codex-telemetry-research-handoff.md:92`, `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md:97,112,148` |
| `0ca7439866e8f2b6` | `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md:112,123,150` |

The six files were last committed between 2026-09-28 and 2026-10-06 (`git log -1` per file). The prose around two of the ids calls them bugs.

## Reproduction

```
codescout audit-doc-refs --fail-on high --no-emit-tracker --json
```

exits 1; `jq -r '.findings[]|select(.severity=="high")|[.md_file,.md_line,.raw_ref,.verdict]|@tsv'` on the output gives the 11 rows above. Then `doc(action="get", id="<id>")` for each id answers "unknown artifact id".

## Environment

codescout `experiments`, catalog on this machine, 2026-11-02.

## Root cause

Not established. The error text of `doc(get)` says a `doc(move)` re-keys an id (id = sha256(abs_path)). Several of these bugs were archived in the same weeks, for example by commits `8dfc251e4` (2026-09-28) and `4b6d1c419` (2026-10-01). That is the tool's general statement, not shown for these ids: `git log -S` for `1fb2cb7b4614e8b5` and for `id: 82973a1e83aa069f` under `docs/issues` and `.codescout/memories` returns no commit.

## Evidence

The `jq` output above, five `doc(get)` answers, and `git log -S` results (empty).

## Hypotheses tried

- The findings come from my own commits of 2026-11-02: no. The six files were last committed in September and October.

## Fix

Not done.

## Tests added

N/A. Nothing is fixed.

## Workarounds

Scope the audit to the touched files with `--paths`.

## Resume

Found while checking item 1 of the open list in the usage audit (`docs/usage-reports/2026-11-02-usage-analysis.md`). Only filed. Not checked: whether another machine's catalog knows these ids, whether an archived file can be matched to each id, and what the previous full-repository run reported.

## References

- `docs/trackers/doc-ref-audit.md`

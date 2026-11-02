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
unverified: Cause found for three of five ids; the other two are not traced. The audit count was read once on 2026-11-02. Whether other archived bugs have unrepointed citations in places the audit does not read was not checked.
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

Found for three of the five ids, not for two.

**Three ids are the pre-archive ids of archived bug files**, as the `doc(get)` error text says: an id is `sha256(abs_path)`, so moving a bug file into `docs/issues/archive/` re-keys it and every citation of the old id resolves to nothing. Verified by recomputing `sha256` of the file's path before the move for every archived file and comparing: `1fb2cb7b4614e8b5` was `docs/issues/2026-09-25-the-judge-channel-check-does-not-see-synced-claude-ai-skills.md` (now `ed9cfbc46f8d9a9e`), `82973a1e83aa069f` was `docs/issues/2026-09-20-agent-id-null-conflates-pre-migration-with-main-session.md` (now `46260d66a62aecf1`), `0ca7439866e8f2b6` was `docs/issues/2026-09-20-observation-window-yields-zero-prospective-samples.md` (now `57c2f7282386d052`). The citations were not repointed when those files moved, which the archive procedure asks for in the same commit.

**Two ids are not traced:** `626ce2d3fa74dd7c` and `fb7592ff7c344255`, in `docs/superpowers/plans/2026-09-09-doctor-per-project-isolation.md:1219`. The sentence calls them "foreign findings checked", that is artifacts of other repositories in the catalog at the time. No path produces either id: I recomputed `sha256` over 482,594 markdown files under `~/work`, `~/Documents` and `~/agents` (with and without `archive/` in the path) and found no match, and neither id is in this machine's catalog. Two other ids in the same list, `690f130645515497` and `8d93149691eb85fe`, do resolve here.

## Evidence

The `jq` output above, five `doc(get)` answers, and `git log -S` results (empty).

## Hypotheses tried

- The findings come from my own commits of 2026-11-02: no. The six files were last committed in September and October.

## Fix

Done for the three traced ids, on 2026-11-02, through the catalog: `doc(update_entry)` for the `RC-39` row of `docs/trackers/review-catches.md` (its `bug` field; the Index row resynced by itself), and `doc(update, body_edits)` with the new id in `docs/evals/phase1b-local-classifier-preregistration.md`, `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md` and `docs/trackers/2026-09-21-codex-telemetry-research-handoff.md`. A whole-repository `audit-doc-refs --fail-on high` then reports 2 high findings instead of 11, both for the untraced ids. Not done: those two. Options, none chosen: fence the list of ids in the plan as a deliberate mention (the audit then reports it at medium), or delete the two ids from the sentence. Both change prose of a dated plan, so I left it.

## Tests added

N/A. Nothing is fixed.

## Workarounds

Scope the audit to the touched files with `--paths`.

## Resume

Open for two ids. The three traced ones are repointed; the whole-repository audit reports 2 high findings, both in the plan at line 1219. Nothing guards the cause: a bug file archived without repointing its id citations still breaks the audit only on a whole-repository run, because the per-commit hook (`dead-artifact-ids`) checks the commit's own lines. Not checked: whether another archived bug has the same unrepointed citations in a place the audit does not read.

## References

- `docs/trackers/doc-ref-audit.md`

---
id: '788740c323324877'
kind: bug
status: fixed
title: 'BUG: a tracked symlink with an absolute target makes audit-doc-refs report a high finding on every clean checkout'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- ci
- docs
closed: 2026-11-01
opened: 2026-11-01
severity: medium
unverified: Verified RED and GREEN in a clean clone with the symlink broken and a fresh HOME. The tracked symlink itself, which dangles on every clone, is not changed; only the one citation of it is. The runner result after the fix is not yet observed.
---

## Summary

`.claude/skills/claude-traces` is a tracked symlink whose target is `/home/marius/agents/llm-proxy/.claude/skills/claude-traces`, a path that exists on one machine. `scripts/measure/README.md:16` cites the link as a file path. On a clean checkout the target is absent, so `codescout audit-doc-refs` classes the citation `missing` with severity `high`, and the CI job `Audit Doc Refs` (`--fail-on high`) exits 1.

## Class fit

`cluster/repro-env-diverges-from-gate-env`: the citation resolves on the author's machine, where the symlink target exists, and not on the runner. It differs from the class's other members in that the divergence is a file the repository tracks but cannot carry: the link is in git, its target is not.

## Symptom (Effect)

CI run 41322768679, job `Audit Doc Refs`: the JSON report has `"exit_code": 1` and one high finding, `scripts/measure/README.md` line 16, `raw_ref` `.claude/skills/claude-traces`, `verdict` `missing`, `severity_reason` `policy_default`.

## Reproduction

In a clone of `experiments`, point the link at a missing target (`ln -sfn /nonexistent .claude/skills/claude-traces`) and run, with a fresh `HOME`: `LIBRARIAN_WORKSPACE=<empty file> codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .`

Before the fix: exit 1, one high finding (the README line). After: exit 0, no high finding.

## Environment

codescout `experiments`, release binary built on this machine; the clone at a scratch path; `HOME` an empty directory; `RUSTUP_HOME` and `CARGO_HOME` kept so the language servers start.

## Root cause

The report shows findings most-severe-first, capped at 50 (`overflow.shown`), so the 50 shown on CI contain every high one: CI has exactly one. A run with the author's real `HOME` shows 11 more highs (`artifact_missing`), which come from that machine's librarian catalog and are not present on CI; they are a trap for anyone who tries to reproduce this locally.

## Evidence

Audit JSON before and after, in a clean clone: 1 high finding, then 0.

## Hypotheses tried

That the 11 extra highs seen locally also fail CI: refuted by the sort order in `build_response` (`src/librarian/tools/audit_doc_refs/mod.rs`) and by the CI report, which lists one high within its 50.

## Fix

The README now describes the link as the `claude-traces` skill link under `.claude/skills/` and says that its target is an absolute path on the author's machine.

Fixed by `f6ff984a9` on `experiments` (patch-id `0f61ddd87f9d5db46d1b7f01738d8ba2d2f684ec`). The full local gate was green on `9cf788035`. The runner result after the fix is not yet observed.

## Tests added

None new: the CI job is the test. RED and GREEN are the two clean-clone runs above.

## Workarounds

None.

## Resume

1. Read `Audit Doc Refs` on the first CI run with the fix. Expect exit 0.
2. Consider removing the tracked symlink or making it relative: any new citation of the path will fail the same way.

## References

- CI run 41322768679.
- `scripts/measure/README.md`, `src/cli/audit_doc_refs.rs` (the note on why there is no `--reindex`).

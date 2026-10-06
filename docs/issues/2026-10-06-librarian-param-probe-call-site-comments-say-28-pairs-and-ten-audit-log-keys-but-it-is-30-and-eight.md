---
id: '52c6091a2e9a955d'
kind: bug
status: open
title: 'BUG: the librarian param-probe call site''s comments say 28 pairs and ten audit_log keys, but the sweep covers 30 and the schema has eight'
tags:
- librarian
- param-probe
- doc-drift
- cluster/doc-contradicted-by-code
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-09-24-residual-param-probe-reports-skipped-keys.md
severity: low
---

# BUG: the librarian param-probe call site's comments contradict the sweep: `floor` says 28 pairs and the sweep covers 30, and "Ten are labelled `audit_log:`" where the schema has eight

## Summary

Two comments at `src/librarian/tools/librarian.rs:241-250` state figures that the code contradicts. The `floor` comment says the figure is "measured not chosen: 28 pairs, read 2026-09-09". The sweep covers 30 pairs today. The pin comment says "Ten are labelled `audit_log:`". The schema has eight.

## Symptom (Effect)

The floor is a lower bound, so the test passes. The margin between floor and count is two. `assert_all_honored` documents that margin as "exactly the number of labels that can go missing in silence" (`src/tools/param_probe.rs:326-345`). Two pairs can therefore disappear and the test stays green.

Measured 2026-10-06 at `10e935e3`, in a throwaway detached worktree where the floor was raised to 100000:

```
librarian: expected the sweep to cover at least 100000 labelled keys, covered 30 — ... (coverage: 30 pair(s) probed, 0 key(s) skipped as accepts_any_json, 13 key(s) skipped as unlabelled)
```

The comment at `librarian.rs:249` says "Ten are labelled `audit_log:`". A grep for descriptions that start with `audit_log:` finds eight: `tbl`, `row_id`, `actor`, `op`, `since`, `until` (lines 113-118), `prune_before_ms` (line 120) and `export` (line 122).

The count is consistent with eight, not ten. The thirteen pinned keys are the eight `audit_log:` keys plus `write`, `root`, `confirm`, `old_root` and `new_root`. With "ten", the comment would describe fifteen.

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

1. In a scratch worktree, change `28,` at `src/librarian/tools/librarian.rs:246` to `100000,`.
2. Run `scripts/with-slot.sh cargo test --lib every_action_labelled_schema_key_is_honored_by_that_action`.
3. Read the panic text above. Delete the worktree.
4. Run `grep -c '"description": "audit_log:' src/librarian/tools/librarian.rs`. It prints 8.

## Environment

Linux, `experiments` at `10e935e3`.

## Root cause

The floor was set from one reading on 2026-09-09 (the comment says so). The count is 30 today, so something changed after that reading. Inferred, not measured: the schema gained labelled actions or keys. The "Ten" figure does not match the schema now, and when it was last true is not established. `docs/issues/archive/2026-09-09-param-probe-checks-one-action-per-shared-key.md` says a floor "carrying a per-key figure" is the defect, and that it is set at the measurement and moved on purpose.

Measured 2026-10-06 (the throwaway run). The origin of "Ten" is not established: the figure may have been wrong when written.

## Evidence

The archived record `docs/issues/archive/2026-09-24-residual-param-probe-reports-skipped-keys.md` lists both in its Resume: "the sweep was reported to measure 30 at this commit. The floor was not set at the current measurement" and "the schema carries eight keys with that label".

## Hypotheses tried

None needed.

## Fix

Not started. Set the floor to 30 and re-date its comment. Change "Ten" to "Eight". Both edits are comments or constants in one test.

## Tests added

N/A — not fixed. The floor margin is itself the exposure, so a regression test would have to compute the expected pair count, which the sweep already does.

## Workarounds

None needed.

## Resume

Do this edit together with the `audit_log` follow-up: that change moves the floor again.

## References

- `docs/issues/archive/2026-09-24-residual-param-probe-reports-skipped-keys.md`, Resume bullets 2 and 4.
- Sibling: `docs/issues/2026-10-06-librarian-param-honored-sweep-leaves-audit-log-and-five-prose-labelled-keys-unprobed.md`.
- Cluster `IC-11`: the prose was true or plausible when written and the code moved. The class names a capability the code gained, so the fit is loose: here the code gained pairs and the comment kept the old count.

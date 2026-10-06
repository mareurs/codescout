---
id: '9473cc074ff7e799'
kind: bug
status: open
title: 'BUG: the librarian param-honored sweep never probes the audit_log action or five prose-labelled keys, so thirteen schema keys are unguarded'
tags:
- librarian
- param-probe
- test-coverage
- cluster/guard-narrower-than-its-name
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-09-24-residual-param-probe-reports-skipped-keys.md
severity: low
---

# BUG: the librarian param-honored sweep never probes the `audit_log` action or five prose-labelled keys, so thirteen schema keys are unguarded

## Summary

`every_action_labelled_schema_key_is_honored_by_that_action` is named for every labelled key of the `librarian` tool. It sweeps ten actions. The tool has eleven. The `audit_log` action is missing from the sweep, and five more keys carry labels that name no action. Thirteen keys are therefore never checked for the "accepted, then silently dropped" shape. The test records the thirteen in a pin, so it is green.

## Symptom (Effect)

The sweep reports its own gap. Measured 2026-10-06 at `10e935e3`, in a throwaway detached worktree where the floor was raised to an impossible value:

```
librarian: expected the sweep to cover at least 100000 labelled keys, covered 30 — ... (coverage: 30 pair(s) probed, 0 key(s) skipped as accepts_any_json, 13 key(s) skipped as unlabelled)
```

The thirteen keys are pinned at `src/librarian/tools/librarian.rs:255-269`: `actor`, `confirm`, `export`, `new_root`, `old_root`, `op`, `prune_before_ms`, `root`, `row_id`, `since`, `tbl`, `until`, `write`.

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

1. Read `src/librarian/tools/librarian.rs:57`. The schema `enum` lists eleven actions, including `audit_log`.
2. Read `src/librarian/tools/librarian.rs:221`. `spec.actions` lists ten. `audit_log` is not there, although `Librarian::call` routes it at line 153.
3. Count the schema descriptions that start with `audit_log:` (lines 113-118, 120, 122). There are eight.
4. Read the other five keys. `write` opens `legibility_scan (default true): …`. `root` and `confirm` open `doctor fix=…`. `old_root` and `new_root` open `For fix=rehome:`. Each label names no action.

## Environment

Linux, `experiments` at `10e935e3`, crate `codescout`, `--lib` test run through `scripts/with-slot.sh`.

## Root cause

The sweep (`crate::tools::param_probe::sweep`) probes a key only when a label token names an action in `spec.actions`. A key whose label names no listed action is skipped and dropped from both `checked` and `unhonored`. `src/tools/param_probe.rs` `assert_all_honored` therefore pins the skipped keys by name and compares the pin for set equality. That makes the debt visible, but it does not close it.

Measured 2026-10-06: the run above. The mechanism is read from `src/tools/param_probe.rs:326-403` and `src/librarian/tools/librarian.rs:221-269`.

Not checked: whether any of the thirteen keys is in fact dropped today. The `audit_log` `Args` type was not read. The gap is that nothing would notice if one were.

## Evidence

The comment at `librarian.rs:247-254` says the thirteen are "that debt made visible rather than a pass". The archived record `docs/issues/archive/2026-09-24-residual-param-probe-reports-skipped-keys.md` lists the same keys under "Residual follow-ups, listed and not filed".

## Hypotheses tried

None needed. The skip is by design and is recorded in the test itself.

## Fix

Not started. Options:

- Add `audit_log` to `spec.actions` with a `required` arm. This moves `floor`.
- Relabel `write`, `root`, `confirm`, `old_root` and `new_root` to a bare `<action>:` form, or give each its own action label.
- Each cleared key must also leave the pin. The pin reds when a probed key is still listed.

## Tests added

N/A — not fixed.

## Workarounds

None needed. Read the pin when a `librarian` key is added or renamed.

## Resume

Pick the `audit_log` action first. It holds eight of the thirteen keys.

## References

- `docs/issues/archive/2026-09-24-residual-param-probe-reports-skipped-keys.md`, Resume bullet 1. The follow-up was listed there and never filed.
- `docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md`. It fixed the earlier form of the same gap.
- Cluster `IC-14`: the test is named for every labelled key, and covers ten of eleven actions.

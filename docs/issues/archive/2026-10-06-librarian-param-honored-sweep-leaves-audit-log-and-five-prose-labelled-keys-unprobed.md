---
id: da1fd0fd414e3e94
kind: bug
status: fixed
title: 'BUG: the librarian param-honored sweep never probes the audit_log action or five prose-labelled keys, so thirteen schema keys are unguarded'
tags:
- librarian
- param-probe
- test-coverage
- cluster/guard-narrower-than-its-name
closed: 2026-10-10
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-09-24-residual-param-probe-reports-skipped-keys.md
severity: low
unverified: STANDING — the generic param-honored sweep cannot probe `audit_log`, because every librarian call writes an audit row and so any two responses differ; the eight keys are guarded by a dedicated unit test and pinned as unprobed in the sweep.
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

## Fix provenance (2026-10-10)

This section supersedes `## Fix`, `## Tests added` and `## Resume` above, which describe the state before the fix.

- **SHA:** `f1f56b05` (`experiments`)
- **patch-id:** `60987f4eaafa81d80eb292ae448adfff19676e8d`
- **SHA:** `a3a317c0` (`experiments`)
- **patch-id:** `47bd6bdc828e47c06279ba7edcddb89db54ca34a`

The question this bug left open, whether any of the thirteen keys is actually dropped, was checked on 2026-10-10 by passing an ill-typed value to each. The answer splits.

- **The five prose-labelled keys are honored.** `legibility_scan:write`, `link_scan:write`, `doctor:root`, `doctor:old_root`, `doctor:new_root`, `doctor:confirm` and `merge_worktree:root` each returned `invalid type: sequence` from typed deserialization. They were only unreadable to the sweep, because their descriptions put prose before the first colon. They now carry `<action>/<action>:` labels, and so does `limit` (for the four actions that read it). The sweep covers 39 action/key pairs, up from 30, and the floor is 39.
- **The eight `audit_log` keys were NOT honored.** `audit_log::call` read each with `args.get(k).and_then(Value::as_*)`, and a wrong-typed value became "absent". `export="true"` ran a query in place of an export, and `prune_before_ms="123"` ran a query that read like a pruning dry run (IC-15). `reject_ill_typed_args` now refuses a present, non-null argument of the wrong type, by name, with a corrected call. Null still means absent.

**`audit_log` is deliberately still not in the generic sweep.** Every librarian call writes an audit row, so any two `audit_log` responses differ in `table_total`, and the sweep's base-versus-probe comparison is true whether or not the key was read. With the action added, the eight keys stayed green with the validation removed, which is a pass that cannot fail. The eight keys are pinned as unprobed, with that reason in the comment, and are guarded instead by `audit_log::tests::a_wrong_typed_argument_is_refused_by_name_and_never_read_as_absent` (ten wrong-typed cases, each checked for the key's name and a corrected call) and `well_typed_and_null_arguments_still_run` (the twin). Mutation checks, each applied and restored byte-identical: removing the validation call fails the first test; weakening the `limit` rule to "is a number" fails its `limit = -1` case.

`a3a317c0` trims the description additions to the labels alone and ratchets the tool-surface budget 57_491 to 57_607 (+116, measured). The full gate ran green on the merged tree (lean 4471, default 6744, Python 1010 passed, 0 failed).

## References

- `docs/issues/archive/2026-09-24-residual-param-probe-reports-skipped-keys.md`, Resume bullet 1. The follow-up was listed there and never filed.
- `docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md`. It fixed the earlier form of the same gap.
- Cluster `IC-14`: the test is named for every labelled key, and covers ten of eleven actions.

---
id: c77c15b68a60e126
kind: bug
status: open
title: 'BUG: three cluster-ledger rules are tested but not enforced at commit time'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`tests/issue_clusters.rs` declares three rules as **owed by the commit hook and not implemented
there**. They are corpus invariants a single commit can break, and
`scripts/pre-commit-ledger-counts.py` does not check any of them:

| rule | what it asserts | why the hook lacks it |
|---|---|---|
| `every_declared_class_has_an_index_row` | every `**Slug:**` declaration has an Index row | the hook parses Index rows only for counts; no count-free row parser exists there |
| `the_index_file_holds_no_class_sections` | `docs/trackers/issue-clusters.md` holds no `## IC-N —` section | nothing; it is cheap and was grouped rather than shipped alone |
| `no_mechanism_status_is_a_bare_verdict` | no `**Mechanism status:**` is a bare verdict | needs the mechanism-status parser ported to Python |

**This is a declared divergence, not a silent one, and the distinction is the whole point.** Each
sits in `NOT_HOOK_OWED` with the reason `OWED, not yet implemented` and a pointer here, so
`every_cluster_rule_is_hook_owed_or_exempt` passes — the mechanism shipped in `51dd7c15`'s
successor records that the question was asked and answered *later*, rather than never asked. An
entry reading `exempt` would close the question; these do not.

## Symptom (Effect)

A commit that adds a `**Slug:**` without an Index row, moves a class section into the Index file,
or writes a bare `**Mechanism status:** none yet` passes the commit path and reds
`cargo test --test issue_clusters` for every other session sharing the checkout. Same shape as
the one-tag rule before 2026-09-11 — the cost lands on people who did not write it.

## Reproduction

1. Add `**Slug:** \`cluster/no-index-row-for-me\`` to a per-class file without adding an Index row.
2. `python3 scripts/pre-commit-ledger-counts.py --source=worktree` — observe **exit 0**.
3. `cargo test --test issue_clusters every_declared_class_has_an_index_row` — observe **FAILED**.

## Environment

`experiments`, 2026-09-11. Both sides are at the commit that introduced `HOOK_RULES` / `HOOK_OWED`.

## Root cause

The hook was written to carry the rules that were cheap at the time it was written, and until
2026-09-11 nothing compared the two rule sets, so the omissions were invisible rather than
decided. The comparison now exists and makes them visible; implementing them is the remaining
work.

## Fix

Port each rule into `scripts/pre-commit-ledger-counts.py`, move its name from `NOT_HOOK_OWED` to
`HOOK_OWED`, and add it to the script's `HOOK_RULES`. `the_hook_enforces_every_rule_it_declares`
reds until all three sides agree, so the move cannot be done half-way.

Take `the_index_file_holds_no_class_sections` first — it is a line-anchored `## IC-` check with no
new parser — and `no_mechanism_status_is_a_bare_verdict` last, since it needs the most porting.

**Each ported rule owes its own discrimination test**, in the shape the existing ones use: a
fixture with a known answer fed through the PYTHON implementation, so a check that silently stops
matching is not mistaken for a clean corpus. A ported rule without one converts a Rust guard into
a pair where one half is decoration.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet.

## Resume

Run the reproduction first — it takes two commands and tells you which of the three you are
actually looking at. Then port one rule end to end (script check + `HOOK_RULES` + `HOOK_OWED` +
discrimination test) before starting the next: the three share no code, so a partial port of all
three is three unfinished edits rather than one finished one.

## References

- Declared by `NOT_HOOK_OWED` in `tests/issue_clusters.rs`, whose entries name this file.
- Filed 2026-09-11 while fixing
  `docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`,
  which built the rule-set comparison that made these three visible.
- `docs/trackers/issue-clusters.md` § `IC-14` (`guard-narrower-than-its-name`).

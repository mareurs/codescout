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
| ~~`the_index_file_holds_no_class_sections`~~ | `docs/trackers/issue-clusters.md` holds no `## IC-N —` section | **PORTED** `05cda53e` — was: nothing; cheap, and grouped rather than shipped alone |
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

**Progress: 1 of 3 ported.**

| rule | state |
|---|---|
| `the_index_file_holds_no_class_sections` | ported — SHA `05cda53e`, patch-id `11704167c6d3947e7cab02e3371f28a132a48d55` |
| `every_declared_class_has_an_index_row` | not started |
| `no_mechanism_status_is_a_bare_verdict` | not started |

The bug closes when all three are in `HOOK_RULES`; the file stays open until then.

**One thing the port learned that the plan above does not say, and the next two rules will
meet it too.** The Python check must be fed the **Index file alone** — `read(LEDGER, source)`,
never the `read_ledger()` result the surrounding code already holds. `read_ledger()` returns
the Index concatenated with every class file, and each class file opens with its own
`## IC-N —` heading, so the joined text matches once per class on a perfectly healthy corpus.
Mutating that one argument was one of the three kills: the hook then exits 1 on a clean tree,
naming all 23 sections. The Rust twin reads `repo_root().join(LEDGER)` and cannot make this
mistake; the Python one is handed the wrong value by default.

## Tests added

For `the_index_file_holds_no_class_sections` (`05cda53e`):

- `the_index_section_scan_discriminates` (`tests/issue_clusters.rs`) — feeds
  `INDEX_SECTION_FIXTURE` to the Rust scan. Planted killers: the template placeholder
  `## IC-N —`, a non-ASCII digit, a mid-line mention, a `### ` heading, and a two-digit id.
- `the_hook_script_agrees_on_the_index_section_scan` (`tests/issue_clusters.rs`) — the same
  fixture through `--fixture-index-sections`, asserting agreement **and** that the Python
  answer is non-empty.

The scan was extracted out of the corpus test into `index_class_sections` so both share one
derivation. Three mutations run on the production path, all killed:

| mutation | kill |
|---|---|
| drop `isascii()` in Python | parity test reds — Python alone matches `## IC-٣` |
| pass `ledger` instead of the Index file | hook exits 1 on a clean tree, all 23 sections named |
| `is_ascii_digit` → `is_numeric` in Rust | both new tests red, opposite direction |

**A blind spot pinned rather than closed:** a titleless `## IC-8` is not a finding on either
side, because both require the space separating id from title. Nothing writes that shape —
`append_entry` always emits `## IC-N — <title>` — so it is pinned in the fixture to stop a
future widening happening on one side silently.

## Resume

**Next: `every_declared_class_has_an_index_row`** (the middle rule; leave
`no_mechanism_status_is_a_bare_verdict` for last, it needs the most parser).

The repro is read-only and takes one command — do not use the arming one in § Reproduction
unless you need the consequence:

    python3 scripts/pre-commit-ledger-counts.py --rules

The script PRINTS `HOOK_RULES`, so the divergence is visible without writing anything to a
shared tree. Step 1 of § Reproduction adds a `**Slug:**` to a real file, which reds
`cargo test --test issue_clusters` for every other session until you revert it.

The end-to-end shape, from the one already ported (`05cda53e` is the worked example):

1. Add the rule id to `HOOK_OWED` in `tests/issue_clusters.rs` **first** and run
   `cargo test --test issue_clusters the_hook_enforces_every_rule_it_declares` — it reds, and
   that red is the porting contract stating itself.
2. Implement the check in `scripts/pre-commit-ledger-counts.py`, add the id to `HOOK_RULES`,
   and remove the `NOT_HOOK_OWED` entry.
3. Add a `--fixture-<rule>` stdin arm and the paired discrimination test; classify that new
   test into `NOT_HOOK_OWED` or `every_cluster_rule_is_hook_owed_or_exempt` refuses it.
4. Mutate the Python, not the fixture.

Rule 2 differs from rule 1 in one way worth knowing before starting: it needs a **count-free
Index-row parser**, which the hook does not have — `parse_index_counts` reads rows only to
extract counts. The Rust side already has one (`parse_index_rows` / `missing_index_rows`,
`tests/issue_clusters.rs`), including its `unclassified` exemption, so the port is a
translation rather than a design.

## References

- Declared by `NOT_HOOK_OWED` in `tests/issue_clusters.rs`, whose entries name this file.
- Filed 2026-09-11 while fixing
  `docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`,
  which built the rule-set comparison that made these three visible.
- `docs/trackers/issue-clusters.md` § `IC-14` (`guard-narrower-than-its-name`).

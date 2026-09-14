---
id: ef7b2f22c40a458e
kind: bug
status: fixed
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
| ~~`every_declared_class_has_an_index_row`~~ | every `**Slug:**` declaration has an Index row | **PORTED** `05dc2a4a` — was: the hook parses Index rows only for counts; no count-free row parser existed there |
| ~~`the_index_file_holds_no_class_sections`~~ | `docs/trackers/issue-clusters.md` holds no `## IC-N —` section | **PORTED** `05cda53e` — was: nothing; cheap, and grouped rather than shipped alone |
| ~~`no_mechanism_status_is_a_bare_verdict`~~ | no `**Mechanism status:**` is a bare verdict | **PORTED** — was: needs the mechanism-status parser ported to Python |

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

Ported. `HOOK_RULES` and `HOOK_OWED + HOOK_ONLY` are equal at **8**, and `NOT_HOOK_OWED`
holds no `OWED, not yet implemented` entry — every rule the gate enforces, the commit path
enforces too.

| rule | SHA | patch-id |
|---|---|---|
| `the_index_file_holds_no_class_sections` | `05cda53e` | `11704167c6d3947e7cab02e3371f28a132a48d55` |
| `every_declared_class_has_an_index_row` | `05dc2a4a` | `0a71140224e14d9a8dcbea670dc4043bb96b8e3f` |
| `no_mechanism_status_is_a_bare_verdict` | `47f31b1c` | `2cbfb1ec06c232a90a384fa7744cfb7e114de9a5` |

The ordering the plan prescribed held up: rule 1 needed no new parser, rule 2 needed a
count-free row parser, rule 3 needed two functions and the most care. Each was ported end to
end before the next was started.

**Three things a later porter of a similar rule should have, none of which were in the plan:**

- **Which text each rule reads is not uniform, and getting it wrong is silent in one direction
  and deafening in the other.** CHECK 5 reads the **Index file alone** (`read(LEDGER, source)`);
  CHECK 6 and CHECK 7 read the **concatenation** (`read_ledger`). Feeding CHECK 5 the
  concatenation refuses every commit in the repo, naming all 23 class sections — loud, and a
  kept mutation. The Rust twins cannot make this mistake: they read distinct helpers.
- **Read-only reproduction.** `--rules` prints `HOOK_RULES`, so the divergence is observable
  without writing to a shared tree. § Reproduction's step 1 arms a red for every other session.
- **Write the mutation down before running it.** Rule 3's parenthetical-stripping arm was
  *unkillable* by the fixture as first written — `IC-4` (`shipped (partial)`) is flagged with
  the strip and without it, both remainders being under the threshold. Only a qualifier long
  enough to pass FOR a basis discriminates, so `IC-10` was added. Nothing but composing the
  mutation in advance would have surfaced that; every assertion was green.
## Tests added

Nine mutations on the production path across the three rules, all killed, each reverted and the
files confirmed byte-identical by `sha256sum -c`.

**Rule 1 — `the_index_file_holds_no_class_sections` (`05cda53e`)**

- `the_index_section_scan_discriminates`, `the_hook_script_agrees_on_the_index_section_scan`
  (`tests/issue_clusters.rs`), fixture `INDEX_SECTION_FIXTURE`.
- The scan was extracted from the corpus test into `index_class_sections` so both share one
  derivation.

| mutation | kill |
|---|---|
| drop `isascii()` in Python | parity reds — Python alone matches `## IC-٣` |
| pass `ledger` instead of the Index file | hook exits 1 on a clean tree, all 23 sections named |
| `is_ascii_digit` → `is_numeric` in Rust | both new tests red, opposite direction |

**Rule 2 — `every_declared_class_has_an_index_row` (`05dc2a4a`)**

- `the_hook_script_agrees_on_the_index_row_scan`, fixture `INDEX_ROW_FIXTURE` (a mini-ledger, so
  `valid` is derived from the fixture rather than the live corpus).
- **Closes a gap that predates the port:** `the_index_count_parser_discriminates` exercises
  `parse_index_counts`, so the count-free `parse_index_rows` had no fixture on either side — only
  the live corpus, where it returns 23 rows whether it reads the slug cell or merely something
  backticked. **That fixture's NAME was the one loose end this bug left:** it was named for the
  row parser while exercising the count one, and renaming it touches `NOT_HOOK_OWED`, so it was
  judged out of scope here rather than forgotten. Closed later the same day on an operator
  go-ahead — the name above is current, and the `NOT_HOOK_OWED` entry moved with it.

| mutation | kill |
|---|---|
| drop the `unclassified` exemption | `missing` gains `unclassified` |
| break on the first backticked cell | `IC-3`'s leading non-slug backtick drops the row |
| accept unbackticked slug cells | `beta-slug` becomes a row, `missing` goes EMPTY — a real gap reported clean |

**Rule 3 — `no_mechanism_status_is_a_bare_verdict` (`47f31b1c`)**

- `the_hook_script_agrees_on_the_mechanism_basis_scan`, fixture `MECHANISM_BASIS_FIXTURE` (ten
  fields plus a fenced template specimen that must NOT be collected).

| mutation | kill |
|---|---|
| drop the fence toggle | the specimen leaks in as `("IC-", "none yet \| designed \| shipped (<what>)")` |
| drop the parenthetical strip | `IC-10` only — see § Fix |
| word-list membership instead of strip-and-measure | catches `IC-1` alone; five bare verdicts pass |

**A blind spot pinned rather than closed** (rule 1): a titleless `## IC-8` is not a finding on
either side, because both require the space separating id from title. Nothing writes that shape.
Pinned in the fixture so a future widening cannot happen on one side silently.
## Resume

N/A — fixed. Gate green on each of the three commits (FMT/CLIPPY/LEAN/DEFAULT all 0; 9751,
9757 and 9759 passed respectively), and each rule's test names were read individually out of
both lanes rather than off a total.
## References

- Declared by `NOT_HOOK_OWED` in `tests/issue_clusters.rs`, whose entries name this file.
- Filed 2026-09-11 while fixing
  `docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`,
  which built the rule-set comparison that made these three visible.
- `docs/trackers/issue-clusters.md` § `IC-14` (`guard-narrower-than-its-name`).

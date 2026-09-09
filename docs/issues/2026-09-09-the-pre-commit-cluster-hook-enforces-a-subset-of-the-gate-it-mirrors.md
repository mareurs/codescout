---
id: fa8b34c2a56148a8
kind: bug
status: open
title: 'BUG: the pre-commit cluster hook enforces a subset of the gate it mirrors, and the test asserting they agree compares only their parsers'
tags:
- cluster/guard-narrower-than-its-name
- hooks
- guards
- issue-clusters
- pre-commit
opened: 2026-09-09
owner: marius
severity: medium
---

## Summary

`scripts/pre-commit-ledger-counts.py` enforces a **strict subset** of what
`tests/issue_clusters.rs` enforces. The hook checks stored counts and the
member-naming growth rule; it does **not** check
`every_open_bug_file_declares_one_known_defect_class` — the "exactly one
`cluster/<slug>` tag per open bug file" invariant. A commit adding a second
cluster tag passes the hook and reds the shared gate for every session in the
checkout.

`tests/issue_clusters.rs::the_hook_script_agrees_with_this_gate` is the test a
reader consults to rule this out, and its **name overstates what it checks**. It
compares the two implementations' *parse logic* — `parse_index_counts` and
`cluster_tags` — not the set of *rules* each side enforces. Its own doc comment
is accurate ("Pointing both at the working tree isolates the thing actually at
risk: the **parse logic**"); the name is what a hurried reader takes, and
"agrees with this gate" reads as rule-parity.

## Symptom (Effect)

Measured live 2026-09-09. Commit `bd7d7eb1` added a second `cluster/` tag to
`docs/issues/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`:

| surface | verdict |
|---|---|
| `pre-commit` hook, all four checks | **Passed** — commit accepted |
| `cargo test --test issue_clusters` at the resulting HEAD | **FAILED** |
| `the_hook_script_agrees_with_this_gate` | **ok** — green throughout |

The red was found by a peer session (`c86ebb51-7ae3-477d-b755-f25db6180782`)
running the suite for their own unrelated work, not by the author, and not by
any gate on the author's path. Repaired at `6697cbd7`.

## Reproduction

1. Add a second `cluster/<slug>` tag to any **open** bug file, through the
   catalog: `doc(action="update", id=…, patch={tags:[…two cluster tags…]})`.
2. `git add` and commit it. The hook's four checks pass, including
   `refuse a stored count, or a class gaining a member it does not name`.
3. `cargo test --test issue_clusters` → **FAILED** on
   `every_open_bug_file_declares_one_known_defect_class`.

Control that makes step 2 mean something: the hook is not inert. In the same
sequence it correctly **refused** an earlier attempt at the same commit, because
the new tag made `IC-3` gain a member its `**Members:**` did not name. So the
hook ran, read the very same tag list, applied one rule to it, and had no
opinion about the count of tags.

## Root cause

Two derivations of the cluster rules exist by deliberate design — the doc
comment on `the_hook_script_agrees_with_this_gate` explains why a `cargo`
invocation in the commit path is unaffordable on a shared `target/` lock, and
names the duplication as "the price". The mitigation it names for drift is that
test. But the test was scoped to the **parsers** the two share, and nothing
compares the **rule sets**. So a rule added to the Rust side is under no
obligation to appear in the hook, and its absence is reported by nothing.

This is the class exactly: a guard whose coverage is narrower than its name, and
where the narrowing is invisible from the name alone.

## Evidence

- `scripts/pre-commit-ledger-counts.py` — grepping the hook for the invariant
  (`exactly one`, `declares_one`, a length comparison over `cluster_tags`)
  returns **two hits, both docstring prose**. `cluster_tags()` exists and is
  called from `counts_by_tag` and `added_member_stems`; neither asks how many
  tags one file carries.
- `tests/issue_clusters.rs:1276` — the agreement test's asserts are over
  `got["claimed"]` (slug, field, n triples) and the tag-style parse. No rule-set
  comparison.
- 21 tests in `tests/issue_clusters.rs`; the hook implements 2 of the rules.
  **Deriving that ratio properly is part of the fix, not this file** — most of
  the 21 are parser-discrimination tests that a commit hook has no business
  running, so a bare "2 of 21" would be the wrong denominator and is
  deliberately not stated as the gap.

## Fix

Not yet fixed. Two shapes, and the second is the one this class asks for:

1. **Teach the hook the one-tag rule.** Cheap — `cluster_tags` already returns
   the list, so the check is its length. Closes this instance and nothing else.
2. **Make the rule sets comparable, so the next divergence reds instead of
   shipping.** Have both sides name their enforced rules (a list of rule ids the
   hook emits under `--json`, checked against the Rust side's set), so a rule
   added to one and not the other is a red rather than a silence. This is what
   the existing test was reached for and is not.

Shape 2 needs a decision about which rules a commit hook *should* carry — a
parser-discrimination test must not be in the hook's set, so the comparison
needs a declared "hook-owed" subset rather than equality. That declaration is
the real work, and it is why this is filed rather than fixed in passing.

Do **not** fix by renaming the test alone. The name is misleading, but renaming
it leaves the divergence unguarded and removes the surface a reader currently
(wrongly) trusts, which is worse than a name that oversells.

**SHA:** N/A — not yet fixed.
**patch-id:** N/A — not yet fixed.

## Tests added

None yet. The discriminating test for shape 1 is a fixture bug file carrying two
valid cluster tags, asserted to be **refused by the hook** — not merely by the
Rust gate, which already catches it. For shape 2, the test is that adding a rule
to the Rust side without adding it to the hook's declared set reds.

## Workarounds

Run `cargo test --test issue_clusters` before committing any change to a bug
file's frontmatter, including one made through the catalog. Note that
`cargo test --lib <NAME>` will **not** do it — that target does not contain the
test and reports `0 passed; N filtered out` with **exit 0**.

## Resume

Filed 2026-09-09 by sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`, whose own
commit `bd7d7eb1` is the instance. Reported by
`c86ebb51-7ae3-477d-b755-f25db6180782`. Repair commit `6697cbd7`.

The author-side lesson that produced the bad commit is separate and belongs with
it: `bd7d7eb1`'s message argued the delta was "Rust COMMENTS and markdown only,
so the two test lanes were not re-run". In a repo with tests **over** markdown,
"docs-only" is not a category that can skip the lanes — and the reason it keeps
reading as one is that the author holds the parameter that would reveal it. They
know the diff is prose; the fact that prose is under test lives in a target they
are not running *because* they believe it is prose. That half is `OB`-shaped and
is not resolved by this bug file; this file is the mechanism half, per
§ *Observer Blindness* position 3 — a check that runs when nobody is worried
beats a lesson addressed to an author who has already concluded they are safe.

## References

- `docs/issues/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`
  — the file whose tags were the instance.
- codescout memory `cargo-test-lib-skips-integration` — the `--lib` false-green
  that made the first verification attempt of this read as passing.

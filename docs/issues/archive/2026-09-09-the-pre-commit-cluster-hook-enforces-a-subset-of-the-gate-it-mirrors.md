---
id: 3260ae2de91940e3
kind: bug
status: fixed
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
`docs/issues/archive/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`:

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

**Both shapes shipped.**

**Shape 1 — the hook enforces the one-tag rule.** `bad_tag_declarations` mirrors `verdict`'s four
arms and emits them in the same words, so a reader who hit one gate recognises the other instead
of debugging a second, differently-worded refusal. The arm that is easy to drop is kept: a file
with **no frontmatter** reports `no cluster/ tag` rather than being skipped — skipping it would
make the worst-formed bug file in the corpus the one the gate is quietest about.

The population is `open_bug_files()`, narrower than the count rules' on purpose: an archived file
predating the closed set would otherwise refuse every commit that touches anything, with no
action available to the committer.

**Shape 2 — the rule sets are comparable.** The script declares `HOOK_RULES` and prints it under
`--rules`; `the_hook_enforces_every_rule_it_declares` asserts **equality** against `HOOK_OWED` +
`HOOK_ONLY`. Equality rather than subset because a rule silently *deleted* from the hook leaves a
smaller set that a subset check still accepts. `HOOK_ONLY` carries
`a_class_gaining_a_member_names_it`, which compares the INDEX against HEAD — a question no
working-tree test can pose, which is the whole reason a hook exists beside the gate.

**The ids are the Rust TEST NAMES**, so neither side needs a translation table. That table would
itself be a surface that drifts, which is the defect one level down.

**The list is not the mechanism, and this is the part § Fix called "the real work".** A
hand-maintained `HOOK_OWED` reproduces this bug exactly one level up — add a rule, forget the
list, and the divergence is unguarded again with nothing reporting it. So
`every_cluster_rule_is_hook_owed_or_exempt` reads this test file's **own source**, enumerates its
`#[test]` functions, and refuses any name declared in neither list. Writing the reason is the
forcing function; the list existing is not. Same shape as
`every_declared_feature_has_a_lane_or_a_reason` (`tests/feature_lanes.rs`), and the same logic the
hook's own growth check already records about itself — a gate that merely demands a line *change*
is satisfied by a trailing space.

**§ Fix asked for a declared "hook-owed" subset rather than equality, and that declaration turned
up three more diverging rules**: `every_declared_class_has_an_index_row`,
`the_index_file_holds_no_class_sections`, `no_mechanism_status_is_a_bare_verdict`. All three are
corpus invariants a single commit can break, and none is in the hook. They carry
`OWED, not yet implemented` and name
`docs/issues/2026-09-11-three-ledger-rules-are-tested-but-not-enforced-at-commit-time.md` rather
than a false `exempt`, which would close the question. A declared divergence pointing at a tracked
bug is not the silence this file is about — but it is not closure either, and the distinction is
why that bug exists rather than a comment.

**The rename was done, and § Fix's prohibition is why it was safe to do.** That prohibition is
against renaming *alone* — it would remove the surface a reader wrongly trusts without closing
the divergence. With `the_hook_enforces_every_rule_it_declares` shipped, a reader looking for rule
parity now finds a test that means it, so `the_hook_script_agrees_with_this_gate` became
`the_hook_script_agrees_on_the_cluster_parsers`. Live citations were updated and historical
narratives left as written, following the precedent `no_index_row_stores_a_count` already sets in
the same file.

Fix SHA: e749c574
Patch-id: 7f0900fae2a6a8827488e84e0653eb6e52043077
## Tests added

Five mutations, each producing an **observed RED** rather than an assertion that exists:

| mutation | what reds |
|---|---|
| a rule dropped from the script's `HOOK_RULES` | `the_hook_enforces_every_rule_it_declares` |
| a new `#[test]` classified in neither list | `every_cluster_rule_is_hook_owed_or_exempt`, unclassified arm |
| a test renamed, its entry left behind | same test, phantom arm |
| a declared name resolving to no test, **in isolation** | same test, phantom arm |
| a placeholder exemption reason (`"n/a"`) | same test, thinness arm |

**The fourth row is there because the third does not establish it.** On a rename both the phantom
and unclassified conditions hold, and whichever asserts first hides the other — so the phantom arm
was only shown reachable by declaring a name that matches no test while every real test stays
classified. The assertions were also reordered to put phantom first, because on a rename *"your
entry points at no test"* is the diagnostic sentence and *"this test is unclassified"* is the
consequence.

Shape 1 was verified against a real bug file in the worktree, both refusal arms: two cluster tags
→ exit 1 naming both, an unknown slug → exit 1 naming it. Restored and re-verified green.

Not covered, and named rather than implied: these prove the two sides agree on **which** rules the
hook carries, never that its implementation of a rule matches the Rust one. For the shared parsers
that is `the_hook_script_agrees_on_the_cluster_parsers`; for the one-tag rule it is that both
sides emit the same four refusal strings, which no assertion checks.
## Workarounds

Run `cargo test --test issue_clusters` before committing any change to a bug
file's frontmatter, including one made through the catalog. Note that
`cargo test --lib <NAME>` will **not** do it — that target does not contain the
test and reports `0 passed; N filtered out` with **exit 0**.

## Resume

Fixed and archived. Two threads continue elsewhere:

- `docs/issues/2026-09-11-three-ledger-rules-are-tested-but-not-enforced-at-commit-time.md` — the
  three rules the new declaration exposed. Read it with this file: closing this one without that
  one would read as the divergence being gone rather than merely visible.
- `docs/issues/2026-09-11-edit-file-reads-a-keyword-in-prose-as-a-symbol-definition.md` — filed
  while writing the hook's new check, whose comment could not be written through `edit_file`
  because the prose contains the word `class`.

The author-side lesson below is unchanged by the fix and is the reason the mechanism was worth
building rather than the note being worth writing.

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

- `docs/issues/archive/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`
  — the file whose tags were the instance.
- codescout memory `cargo-test-lib-skips-integration` — the `--lib` false-green
  that made the first verification attempt of this read as passing.

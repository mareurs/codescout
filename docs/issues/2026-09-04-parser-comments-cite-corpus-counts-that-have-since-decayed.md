---
id: e81b767ba3701316
kind: bug
status: open
title: 'BUG: two sibling cluster-tag parsers justify dual-form support with corpus measurements that have decayed — one now false, one uncheckable'
tags:
- cluster/doc-contradicted-by-code
closed: null
opened: 2026-09-04
owner: marius
related: []
severity: low
---

## Summary

`tests/issue_clusters.rs::cluster_tags` and its Python mirror in
`scripts/pre-commit-ledger-counts.py` each carry a comment justifying **why both YAML tag forms are
read**, and each justification is a measurement of a mutable corpus that nothing re-checks. One has
been falsified — *"ZERO bug files carry a `cluster/` tag in flow style"*, of which there are now
three. The other states two bare integers naming no population, and no population I can construct
reproduces them. **Neither parser is wrong; both annotations are**, and they fail in opposite ways —
the checkable one is false, the false-looking one cannot be checked at all.

## Symptom (Effect)

`scripts/pre-commit-ledger-counts.py:452-455`, verbatim:

```
# Pure over stdin, so `the_hook_script_agrees_on_both_yaml_tag_styles` can feed
# both YAML forms directly. The live corpus cannot exercise the inline arm:
# measured 2026-09-01, ZERO bug files carry a `cluster/` tag in flow style, so a
# corpus-driven agreement check leaves that branch untested however green it is.
```

`tests/issue_clusters.rs:155-156`, verbatim:

```
/// Reads both YAML forms, because the corpus uses both: 20 files write a block sequence
/// under `tags:` and 11 write an inline flow list. Reading only one silently under-reports.
```

No test fails, no gate fires, and both parsers behave correctly. The only observable is that a
reader who checks either claim finds it does not hold.

## Reproduction

At `96574516` on `experiments`. Population is `git ls-files docs/issues` — the INDEX, which is the
definition `tests/issue_clusters.rs` enforces and the one `issue-clusters.md` requires under *How to
mine this*, because a recursive grep reads untracked tool logs back as corpus.

```bash
# Claim 1 — "ZERO bug files carry a cluster/ tag in flow style"
for f in $(git ls-files ':(glob)docs/issues/*.md' ':(glob)docs/issues/archive/*.md'); do
  awk -v F="$f" 'FNR==1{n=0} /^---$/{n++; if(n==2) exit; next}
                 n==1 && /^tags: *\[.*cluster\//{print F}' "$f"
done
```

Measured 2026-09-04 → **3**, not zero:

```
docs/issues/2026-09-02-doc-tool-refs-counts-call-param-pairs-as-documents.md
docs/issues/2026-09-03-reindex-walks-zero-files-in-a-worktree-and-reports-success.md
docs/issues/archive/2026-09-03-markdown-grammar-librarian-guard-has-zero-test-coverage.md
```

```bash
# Claim 2 — "20 files ... block sequence under tags:, 11 ... inline flow list"
git ls-files ':(glob)docs/issues/*.md'         | wc -l   # 93   live
git ls-files ':(glob)docs/issues/archive/*.md' | wc -l   # 580  archive
# files whose `tags:` is a flow list, same awk with /^tags: *\[/ :
#   live 3, archive 142  ->  145 flow, 528 block, over 673 tracked bug files
```

No population yields 20/11: not the live dir (93 / 3), not the archive (580 / 142), not the union
(673 / 145).

## Environment

codescout `experiments`, Linux, `origin/experiments` at `96574516`. Both parsers are exercised by
`tests/issue_clusters.rs` (19 tests, green at time of filing).

## Root cause

**A code comment asserts a measurement of a population the comment does not control, and nothing
re-derives it.** The two instances share that mechanism and diverge only in how they fail:

- `scripts/pre-commit-ledger-counts.py:452-455` names its unit (*"bug files"*) and stamps its date
  (*"measured 2026-09-01"*), so it is falsifiable — and three days later it is false. Bug files
  gained flow-style `cluster/` tags; the comment did not notice, because nothing asks it to.
- `tests/issue_clusters.rs:155-156` states `20` and `11` with **no population and no date**. It
  cannot be falsified, only failed to be reproduced. This is CLAUDE.md's own law — *a count of a
  defect population must arrive with its unit or not at all* — holding against a file three
  directories from where the law is written.

*Measured 2026-09-04, not inferred:* the commands under *Reproduction*, run against
`git ls-files docs/issues`.

**The repo already owns the remedy for this shape, and it is scoped past these two.**
`librarian(action="doctor")` ships `entry_dated_stale` and `entry_conditional_past_due`, which check
exactly this — a dated claim that has passed its validity — but their population is **tracker
entries carrying a `Valid:` line**. The identical claim inside a Rust doc comment or a Python
comment is outside that population, so the check returns a well-formed answer that says nothing
about it. That is `cluster/selector-narrower-than-its-population` as the *mechanism* diagnosis; the
instance itself is filed under IC-11 (see *Hypotheses tried* 3).

## Evidence

### The consequence is not "a stale comment", it is a fixture a reader may correctly delete

The Python comment's *conclusion* — feed both forms from a stdin fixture rather than from the
corpus — is still right, and is arguably more right than when it was written. Its stated *reason* is
now false. A reader who checks the reason, finds the corpus does exercise the inline arm, and
concludes the fixture is redundant would leave `the_hook_script_agrees_on_both_yaml_tag_styles`
resting on **three files**, any of which a routine re-indent removes. The comment currently argues
for the fixture using a fact that, corrected, argues against it.

### Both were found by using the parser, not by reading it

Filed from a session that hand-rolled a `^- cluster/` scan, got a distribution disagreeing with the
catalog by 3, and went looking for why. The scan was wrong and the parsers were right — the
annotations were the only thing left over. Neither claim is reachable by reading the code, because
the code is correct; only re-deriving the numbers surfaces them.

## Hypotheses tried

1. **Hypothesis:** the Rust `20`/`11` counts describe `entry_prefix:` blocks rather than `tags:`.
   **Test:** re-read `tests/issue_clusters.rs:153-184` — `cluster_tags` reads `tags:` and nothing
   else; the doc comment's own sentence says *"under `tags:`"*.
   **Verdict:** rejected.

2. **Hypothesis:** the counts were right for the corpus at the commit that introduced them, so this
   is ordinary decay and only the Python one is a defect.
   **Test:** not run — deliberately. It does not change the finding: a bare integer with no
   population is unverifiable **today by design**, which is the defect, independent of whether it
   was ever accurate.
   **Verdict:** deferred, and recorded so it is not re-walked.

3. **Hypothesis:** this belongs in `cluster/selector-narrower-than-its-population` (IC-18), because
   the decay-checker's population excludes code comments.
   **Test:** compared against IC-11's *Blind party* field — *"the reader, routed to the document by
   its own scope claim and given no signal to cross-check; the author of the prose is not blind —
   they wrote something true"*, which describes both instances exactly.
   **Verdict:** rejected as the *instance* class, kept as the *mechanism* diagnosis. IC-18 names
   what a future gate would have to widen; IC-11 names what these two comments are.
   **Flagged for adjudication:** this is the first IC-11 member whose falsifier is a **mutable data
   corpus** rather than a code change. IC-11's claim sentence says *"the code later gained or lost
   the capability"*; here the code never moved and the corpus did. The rest of the class — prose
   true when written, no systematic check, reader given no signal — fits without strain. Whoever
   re-adjudicates IC-11's spread should decide whether the claim sentence widens or this member
   moves.

## Fix

Not applied — filed on notice per CLAUDE.md's capture-on-notice rule.

Two different repairs, because the two failures are different:

- **`scripts/pre-commit-ledger-counts.py:452-455`** — a two-line edit. Keep the fixture and the
  reason for it, and correct the fact: the corpus now carries 3 flow-style `cluster/` tags, which is
  *fragile* coverage rather than none, so the stdin fixture is what makes the arm reliably
  exercised. State the count with its date and its population, as the surrounding file already does
  elsewhere.
- **`tests/issue_clusters.rs:155-156`** — a decision, not an edit. Either name the population and
  the date (`git ls-files docs/issues` at `<sha>`, N block / M flow), or **drop the numbers**. The
  sentence *"the corpus uses both, and reading only one silently under-reports"* is the load-bearing
  half and needs no figure at all. Prefer dropping: a count in a comment is a claim nothing
  re-derives, which is the defect this file reports.

**Do not "fix" both by re-measuring and writing today's numbers in.** That reproduces the mechanism
with fresher values and resets the clock — the trap CLAUDE.md names as *ship its derivation rather
than its value*.

## Tests added

None. A regression test is the interesting part of this bug and is **deliberately deferred**, not
overlooked: the tractable shape is widening `doctor`'s `entry_dated_stale` population from
`Valid:`-bearing tracker entries to any dated claim, code comments included, which is a
mechanism-design task (`I-N` / `H-N`) rather than a guard on these two lines. Guarding only these
two would be `cluster/assertion-satisfiable-by-accident` — it would pin today's strings and catch no
future instance.

## Workarounds

Do not trust either comment's figures. Re-derive with the commands under *Reproduction* before
citing them, and read the Python comment's *conclusion* (use the stdin fixture) as still correct
while treating its *reason* as void.

## Resume

Decide the two repairs under *Fix* — they are independent and neither depends on the other. Start
with `scripts/pre-commit-ledger-counts.py:452-455`, which is the false one and a two-line edit;
`tests/issue_clusters.rs:155-156` needs a call on drop-vs-qualify and should probably drop.

Then, separately and larger: take the `entry_dated_stale` population question to
`docs/trackers/test-escape-hardening.md` (`I-N`) as a mechanism item, citing this file. Do **not**
fold that into the comment repairs.

## References

- `scripts/pre-commit-ledger-counts.py:452-455` — the falsified measurement.
- `tests/issue_clusters.rs:153-184` — `cluster_tags`; the doc comment carrying `20`/`11`.
- `src/librarian/tools/doctor.rs` — `entry_dated_stale`, `entry_conditional_past_due`: the existing
  decay checks whose population stops at tracker entries.
- `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md` — the class, and the spread this
  member extends.
- `docs/issues/archive/2026-09-04-entry-prefix-guard-is-blind-to-the-flush-block-sequence-form.md` —
  the sibling parser whose *code* had the mirror defect; found in the same session, which is how
  these two comments came to be re-derived at all.


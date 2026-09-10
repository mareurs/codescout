---
id: '987b146635cae607'
kind: bug
status: fixed
title: 'BUG: git ls-files counts index stages, so a merge conflict makes every cited test look declared-three-times'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-10
opened: 2026-09-10
severity: med
---

# BUG: `git ls-files` counts index STAGES, so a merge conflict makes every cited test look declared-three-times

## Summary
`tracked_src_files()` (`tests/result_caps.rs:567-584`) builds its population from `git ls-files
src`, filters to `.rs`, and collects into a `Vec<String>` **with no dedup**. `git ls-files`
without a stage selector prints **one line per index entry**, and an unmerged path has three
(stage 1 base, 2 ours, 3 theirs). So while a merge conflict is unresolved in the index, every
conflicted `.rs` file appears three times in the population, and `resolve_cited_test` — whose job
is to refuse a test name declared in more than one file — sees three "declarations" of every test
that lives in a conflicted file and returns `CitedTestResolution::Ambiguous`.

## Symptom (Effect)
`probed_rows_cite_a_real_test` fails with a diagnosis that is **false** and a remedy that would
**damage correct code**:

```
doctor.caveat_chars: cited_test "a_long_caveat_is_truncated_without_splitting_a_character" is
declared more than once in tracked src/ (src/librarian/tools/doctor.rs,
src/librarian/tools/doctor.rs, src/librarian/tools/doctor.rs) — an unqualified name cannot say
which one this row means, and the resolver would otherwise take the first in `git ls-files src`
order, certifying this cap with another cap's test. Give one declaration a distinguishing name
and cite that.
```

Three things make this expensive rather than merely wrong:

1. **The same file is named three times.** A reader who notices that has the answer; a reader who
   does not is told to rename a test that is declared exactly once.
2. **It fires on PRE-EXISTING, correct rows.** `doctor.caveat_chars` has been `Probed` and
   `Killed` since the 2026-09-03 sweep. Mid-merge it reads as *"your merge broke an unrelated cap
   citation"*, which is the wrong investigation.
3. **The prescribed remedy is harmful.** *"Give one declaration a distinguishing name and cite
   that"* would have the reader rename a uniquely-named test to satisfy a phantom collision, and
   the rename would then be permanent while the collision evaporates on `git add`.

This is `CLAUDE.md` § *Testing Discipline* exactly: *"an alarm can fire, be read by exactly the
right person, and send them somewhere useless — because a suite tests a guard's PREDICATE and
never its REMEDY TEXT."* The predicate (`files.len() > 1`) is correct here. Only the diagnosis
attached to it is wrong, and no assertion in a 67-test file is about the diagnosis.

## Reproduction
Observed live, not constructed, on 2026-09-10 in worktree
`/home/marius/work/claude/codescout/.worktrees/doctor-per-project-isolation`:

1. `git merge experiments` on branch `doctor-per-project-isolation`, producing content conflicts
   in `src/librarian/tools/doctor.rs` and `src/server.rs`.
2. Resolve both files' conflict markers in the WORKING TREE, but do not `git add` them — the
   working tree is now correct and compiles; the index still holds three stages per path.
3. `cargo test --workspace --no-default-features` → `probed_rows_cite_a_real_test` FAILED, naming
   two rows, one of them pre-existing.
4. `git ls-files src | grep -c "librarian/tools/doctor"` → **4** (three stages of `doctor.rs`
   plus `doctor/scope.rs`). `git ls-files --unmerged src` → stages 1/2/3 for both conflicted
   paths.
5. `git add src/librarian/tools/doctor.rs src/server.rs` — no content change whatsoever.
6. Re-run: `git ls-files src | grep -c "librarian/tools/doctor"` → **2**, and the lane goes green
   (`LEAN exit=0`).

Step 5 changes no byte of any `.rs` file, which is what identifies the index rather than the
content as the cause.

## Environment
Rust MCP server `codescout`. Observed on the `doctor-per-project-isolation` worktree during a
merge from `experiments` (`36bfccd3`). Not OS- or transport-specific: the mechanism is
`git ls-files`'s documented per-stage output plus a missing dedup.

## Root cause
`tests/result_caps.rs:567-584`:

```
fn tracked_src_files() -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "src"])
        ...
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|p| p.ends_with(".rs"))
        .map(str::to_owned)
        .collect()
}
```

`git ls-files` reports index ENTRIES, not files. The function's own doc comment says *"Tracked
`.rs` files under `src/`"* and explains the choice of `git ls-files` over a filesystem walk
(*"an untracked file is a peer's in-flight work and gating on it lets one session red another's
build"*) — that reasoning is sound and should be kept; what is missing is only that an unmerged
path is one file with three entries. `--deduplicate` (git ≥ 2.31), or collecting into a
`BTreeSet`, closes it without giving up the index-as-population property.

`resolve_cited_test` then counts distinct FILES by counting the entries it was handed, so three
stages of one file read as three files. The doc comment on `probed_rows_cite_a_real_test`
explicitly frames this refusal as *"this gate's `IC-6` no-disambiguator obligation"* — which is
why this bug is filed under `IC-6`: it is that class biting the guard built for it. The collision
the resolver guards against is *two files sharing a test name*; the collision it actually met is
*one file at three index stages*, a form its namespace has no way to distinguish.

## Evidence
### `git ls-files` output during the conflict
```
100644 dc551977e046919e237a58f7ea215a297e822432 1	src/librarian/tools/doctor.rs
100644 8af7102e069caa7de1c48431af70a90423302396 2	src/librarian/tools/doctor.rs
100644 375df93aff2a837c4536799ae28d50a9c1bdf6d8 3	src/librarian/tools/doctor.rs
100644 9c77a2c4 1	src/server.rs
100644 aaa92501 2	src/server.rs
100644 f3a6f5a3 3	src/server.rs
```

### The same selector shape at other sites — a FLOOR, and deliberately not a claim
`git grep -n "ls-files" -- tests scripts` names further call sites. **One instance is observed
(above); the rest are the same SHAPE and are NOT verified broken**, per `CLAUDE.md`'s *"mutate
once per guarded SITE, not once per feature"*. What separates a hazard from a harmless site is
whether the caller **counts or uniqueness-checks** the population, or merely iterates it — an
iterating reader processes a conflicted file three times and, being idempotent, does not care.

Unverified candidates worth checking, all counting sites:
- `tests/issue_clusters.rs:209` and `:381` — two `ls-files docs/issues` sites; a conflicted bug
  file would count three times.
- `scripts/pre-commit-ledger-counts.py:91`, `:205` — the ledger-count gate's own population.
- `scripts/probe-cluster-census.py`, `scripts/probe-caveat-density.py`,
  `scripts/probe-double-frontmatter.py:66`.

If any of those inflate a count during a docs merge, the number is wrong in the direction nobody
audits — upward — and the census pages that consume it say only that they are a floor.

## Hypotheses tried
1. **Hypothesis:** the new `doctor.outside_roots_display` probe row's marker citation was
   malformed, and the pre-existing `doctor.caveat_chars` failure was collateral from the merge's
   content resolution.
   **Test:** `git ls-files --stage src/librarian/tools/doctor.rs`; count occurrences in
   `git ls-files src`; then `git add` the two paths with no content change and re-run.
   **Verdict:** rejected. Both rows failed for one reason, unrelated to either citation:
   `git add` alone turned the lane green. `result_caps_and_probe_rows_correspond_in_both_directions`
   passed throughout, which had already localised the failure to the resolver rather than to the
   annotation↔row correspondence.

## Fix

**FIXED 2026-09-10.** Both halves, because fixing only the selector leaves the message
ready to misdirect the next reader who reaches `Ambiguous` by some other route.

**(a) The selector.** `tracked_src_files()` now delegates to a new
`tracked_rs_paths(stdout: &str)` that filters `.rs` and collects through a `BTreeSet`, so
a path present at three index stages yields one entry. The split exists so the
de-duplication is reachable by a test without a git checkout in a crafted state — see
Tests added for why that matters. The `git ls-files`-over-a-walk property is untouched;
only the stage multiplicity goes.

**(b) The remedy text.** Extracted into `ambiguity_message(id, cited_test, declarers)`,
which **branches on distinctness** — and the discriminator turned out to be already in the
data and unused. A genuine collision names DIFFERENT files, and a single file declaring
one name twice is already reported as `file (x2)` by `resolve_cited_test`. So the only way
to get the SAME path listed twice is a duplicated population. When that happens the
message now names the cause (*"the POPULATION is duplicated, not the declaration"*), gives
`git ls-files --unmerged src` to confirm and `git add` to fix, and says **do not rename
anything**. The genuine-collision branch keeps the original wording, which was correct for
its own cause all along.

The extraction is what makes the remedy assertable at all — `CLAUDE.md` § *Testing
Discipline* records that a suite tests a guard's predicate and never its remedy text, and
this gate had 54-plus assertions about the predicate and none about the sentence.
## Tests added

Two, one per half, and each **driven red on the production path before being accepted**:

| test | mutation | observed |
|---|---|---|
| `tracked_rs_paths_collapses_a_path_present_at_several_index_stages` | `BTreeSet` → `Vec` in `tracked_rs_paths` | **red, and only this test** (68 pass, 1 fail) |
| `ambiguity_message_distinguishes_a_real_collision_from_a_duplicated_population` | distinctness branch disabled (`if false &&`) | **red, and only this test** — the failure output printed the original misleading message verbatim |

**The dedup test asserts over a fixture string, not the live index, and that is the
design.** A test asserting `tracked_src_files()` returns no duplicates is MONOTONE under
the defect: the live index is clean whenever this gate is normally run, so it passes just
as well with the dedup deleted. Handed real mid-merge `git ls-files` output directly, the
assertion can fail — which the mutation confirms. Its bound is stated on the function: it
proves the dedup works on duplicated input, NOT that `git ls-files` produces duplicates,
which is a claim about git evidenced by the Reproduction table instead.

**The remedy test asserts SHAPE, not prose** — which sentences are present per branch, and
critically the one instruction that must be ABSENT (`!dup.contains("distinguishing
name")`). A heavy rewording that keeps the two branches distinct stays green; collapsing
them reds. That is the regression that actually happened.

The `.md` line in the dedup fixture is load-bearing and annotated as such: it pins that
de-duplicating did not replace the extension filter.

`cargo test --workspace --test result_caps`: **69 passed, 0 failed** (was 67).
## Workarounds
`git add` every resolved conflicted path before running the gate — which the merge flow should do
anyway. If `probed_rows_cite_a_real_test` names a row whose cited test you did not touch, check
`git ls-files --unmerged` before reading the message's advice.

## Resume

**The `result_caps.rs` site is closed. One further site is now VERIFIED to share the
defect — promoted from the unverified floor by reading it, not by assuming:**

`tests/issue_clusters.rs:207` — `tracked_open_bug_files()` runs `git ls-files docs/issues`,
filters, and `collect()`s into a `Vec` with no dedup. Its own doc comment says *"Tracked
bug FILES"*, so a path appearing three times violates the function's own contract, and
dedup is unambiguously correct there rather than a judgement call. Remedy is the same one
line (`BTreeSet` in the collect). Left unfixed deliberately: this bug file scoped itself to
`result_caps.rs`, and the Evidence section's instruction was to decide **per site**, which
is now done for this one — the decision is "yes, fix it", the act is not taken here.

**Still unverified, and not to be swept:** `scripts/pre-commit-ledger-counts.py:91`/`:205`,
`scripts/probe-cluster-census.py`, `scripts/probe-caveat-density.py`,
`scripts/probe-double-frontmatter.py:66`, `tests/e2e/eval_common/proc.rs:22`. What separates
a hazard from a harmless site is whether the caller COUNTS or uniqueness-checks the
population, or merely iterates it — an iterating reader processes a conflicted file three
times and, being idempotent, does not care.
## References
- `tests/result_caps.rs:567-584` (`tracked_src_files`, the selector)
- `tests/result_caps.rs:2688-2700` (`resolve_cited_test` and its first-match doc comment)
- `tests/result_caps.rs:2924-3001` (`probed_rows_cite_a_real_test`, which surfaces it)
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md` (cluster membership)
- `CLAUDE.md` § *Testing Discipline* — the guard-remedy-text law this instantiates

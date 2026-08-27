---
id: '929a2fecbdd3e523'
kind: bug
status: open
title: 'BUG: doctor''s report is still 52% other repos'' rows — Ruling 17 reached two check families, six others still report globally'
tags:
- librarian
- doctor
- scope
- reporting
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-08-27-doctor-reports-other-workspaces-rows-as-violations.md
severity: low
---

## Summary

`442d8b7c` applied Ruling 17 to `abs_path_outside_managed_roots` and took it from
401 findings to 10. It closed the check it named. **It did not close the report:**
measured immediately after, on the same rebuilt binary, `librarian(action="doctor")`
returns 126 violations of which **66 (52%) are still other repos' rows**, now
arriving through six checks the ruling has not reached.

The predecessor bug (`b176ff103ec56c09`, archived) is accurate and complete for its
own scope. This is the same class at the next check, not a regression of that fix.

## Symptom (Effect)

`librarian(action="doctor")` run from `/home/marius/work/claude/codescout`, HEAD
`92ad5155`:

```
check                                       total  foreign
cited_prefix_with_no_definer                   47       33
terminal_status_with_caveat                    43       12
abs_path_outside_managed_roots                 10       10
entry_dated_stale                               8        0
entry_cited_from_outside_but_undeclared         6        0
frontmatter_id_is_not_a_catalog_id              3        3
frontmatter_id_mismatch                         3        3
ledger_defines_nothing                          3        3
worktree_scoped_row                             2        2
entry_without_definition                        1        0
```

`abs_path_outside_managed_roots`' 10 are its legitimate subject — that check exists
to name rows outside managed roots, so a foreign path there is the finding, not a
leak. **Excluding it: 56 of 116 findings (48%) are unactionable from this session.**

The foreign rows span twelve repos:

```
     13 /home/marius/work/stefanini/southpole
     11 /home/marius/work/claude/claude-plugins
      9 /home/marius/work/mirela/backend-kotlin
      7 /home/marius/work/mirela/eduplanner-ui
      7 /home/marius/work/stefanini/invest-europe
      4 /home/marius/work/claude/whatsapp
      4 /home/marius/work/interviews/hackatons
      3 /home/marius/work/personal/home
      2 /home/marius/work/personal/misc
      1 /home/marius/work/claude/changelog-reader
      1 /home/marius/work/claude/pi
      1 /home/marius/work/claude/researcher
```

## Reproduction

```
# 1. rebuild + reconnect so the served binary contains 442d8b7c
cargo rb && /mcp

# 2. from codescout, run the scan
librarian(action="doctor")            # → @tool_* buffer

# 3. pair each check with how many of its findings are foreign.
#    A path that survived relativization as ABSOLUTE is outside the
#    active project — that is the discriminator, and it needs no
#    filesystem access.
awk -F'"' '
/"check":/ {c=$4}
/"path":/  {p=$4; if(c!=""){ tot[c]++; if(p ~ /^\//) foreign[c]++ } }
END{ for(k in tot) printf "%-42s %6d %8d\n", k, tot[k], foreign[k] }
' @tool_<id>
```

## Environment

- Branch `experiments`, HEAD `92ad5155`, 76 commits unpushed.
- Binary rebuilt after `442d8b7c`; confirmed live because
  `abs_path_outside_managed_roots` reads 10, the exact figure that commit's
  message predicts. Not inferred from mtime.
- Catalog is machine-wide: 4000+ artifacts across ~29 distinct `commits.git_root`
  values.

## Root cause

Every check in `doctor::run()` queries the catalog, and **the catalog spans every
repo on the machine.** Scoping the *reported* population to the active project is
opt-in per check, applied so far to exactly two families:

- the entry-validity family — `scan_conditional_past_due`, `scan_dated_stale`,
  `scan_cited_from_outside_undeclared` (Ruling 17, `src/librarian/tools/doctor.rs:229`)
- `scan_artifact_paths` via `known_workspace_roots` (`src/librarian/tools/doctor.rs:1183`,
  added by `442d8b7c`)

Both report **0 foreign rows** in the run above, which is the positive control: the
mechanism works, it has simply not been applied to the other six.

*Measured 2026-08-27 by the awk pairing above; the check-by-check split is not
inferred from reading `run()`.*

## Evidence

### The unactionable-worklist case, which is sharper than the noise case

`frontmatter_id_mismatch` fires 3 times, **all three foreign**. Its own documented
repair is root-scoped — from `librarian`'s tool schema, `fix=repair_frontmatter_id`
rewrites the `id:` of every mismatching file *"for every artifact **UNDER ONE ROOT**
(`root=` or the active project — the catalog spans every repo on the machine and
this writes files)"*.

So the report names three findings that its own repair path, invoked from this
session, will decline to touch. The report and the repair disagree about scope
inside one tool — which is the exact wording the predecessor bug used against
`abs_path_outside_managed_roots` (*"one tool gave two opposite answers to one
question"*).

### The biggest contributor is `cited_prefix_with_no_definer`

33 of 47 (70%). It is the newest check besides `premature_archive_citation` and was
written in the same session, before `442d8b7c` existed — so this is not drift, it is
a check that was born unscoped while the ruling was being applied elsewhere.

### `scan_premature_archive_citation` is unscoped too, with a second hazard — currently empty

The check partitions catalogued bug files into `archived` / `live` sets **keyed on
basename alone**, unioned across every repo (`src/librarian/tools/doctor.rs:3066-3123`).
Two failure modes follow, and the second is worse than noise:

- **Reporting** a foreign repo's premature citation into codescout's run — the same
  defect as above.
- **Cross-repo basename confusion.** A live `docs/issues/X.md` in repo A plus an
  archived `docs/issues/archive/X.md` in repo B silently exonerates A's premature
  citation (false negative), or fires on B's correct one (false positive asserting a
  cause the check cannot know). Either outcome falsifies the check's stated guarantee
  that it is *"wrong in every world"*.

**Measured before claiming it, because a fix that names a population asserts the
population is non-empty (`reconnaissance-patterns:R-117`):** across all **477**
catalogued `docs/issues/` artifacts machine-wide — 451 codescout, 26 foreign across
4 repos — there are **0** basenames that are live in one repo and archived in
another. The only cross-repo duplicate basename is `_TEMPLATE.md`, live in two repos
and never citable as an archive path.

So the second hazard is **latent, not live**. It should be fixed as part of scoping
this check rather than justified by a population that does not exist today.

## Hypotheses tried

1. **Hypothesis:** `442d8b7c` regressed, or its fix did not reach the running binary.
   **Test:** read `summary.by_check` from a live run.
   **Verdict:** rejected — `abs_path_outside_managed_roots` reads 10, matching that
   commit's predicted figure exactly. The fix is live and correct.

2. **Hypothesis:** the predecessor bug already names the other six checks as
   remaining work, so this would be a re-file.
   **Test:** read `b176ff103ec56c09` § Fix, § Tests added, § Resume.
   **Verdict:** rejected — the file is scoped throughout to
   `abs_path_outside_managed_roots`, and § Resume reads *"N/A — fixed,
   live-verified, archived."* No section mentions another check.

3. **Hypothesis:** the foreign rows are an artifact of path relativization rather
   than genuinely foreign.
   **Test:** counted the split on an independent population — 477 `docs/issues/`
   artifacts projected from `artifact(find, scope="all")` — where 451 came back
   relative and 26 absolute, summing exactly.
   **Verdict:** rejected — relativization is the *discriminator*, not a confound.
   An absolute path in a response means the row is outside the active project, by
   the documented `PATH_KEYS` behaviour.

## Fix

**Partially fixed.** The largest contributor is closed; **five checks still report
globally and the bug stays open.**

### Shipped: `cited_prefix_with_no_definer` (2026-08-27)

Applied in the shape `442d8b7c` established — `known_workspace_roots` was not needed,
since this check scopes against the active project's own `git_root` the way the
entry-validity family does rather than against the known-elsewhere set.

Ruling 17 lands on **both halves, in opposite directions**, and that is the design
rather than an implementation detail:

- **Definers stay corpus-wide.** Narrowing `known_prefixes` to the active project
  would make every prefix defined only in a sibling repo — including every cross-repo
  `<repo>:<TOKEN>` citation, which the resolver already and deliberately declines to
  turn into an edge — fire here as *"no definer anywhere in the corpus"*. That
  manufactures false positives out of correct prose: the mirror of the false negatives
  Ruling 17 names on the exposure side.
- **The firing decision is global too; only the report is filtered.** In-project
  citers must clear the same two thresholds, and the remainder is named in the
  violation's own message rather than hidden.
- Scoped-out findings are keyed by the first citer **outside** the project, not by
  `files[0]` — the alphabetically-first citer overall is frequently the in-project one
  that fell below threshold.
- Announced via `catalog_health.cited_prefix_scoped_by_project` + a hint, mirroring
  the two existing scoped families.

**Measured before/after on the SAME corpus** — old binary vs new, both run through the
`codescout doctor --json` CLI. Deliberately *not* compared against the 47/33 figure in
§ Symptom, which was taken an hour and five markdown commits earlier and describes a
different corpus:

| | total | foreign | scoped_out |
|---|---:|---:|---:|
| before (old code) | 48 | 34 | — |
| after (new code) | **14** | **0** | **34** |

48 − 14 = 34, matching `scoped_out` exactly. The whole report goes **128 → 94**.

**Fix commit — record both, they fail differently:**

- SHA `5a7eb3e7596551128c27435a663f3eabce55c71e` on **`experiments`**
- patch-id `6706c5b65a881c9da49c8e27f02a40e36ebf8bde`

### Still open: the other five

`terminal_status_with_caveat` (13 foreign of 44), `frontmatter_id_is_not_a_catalog_id`
(3 of 3), `frontmatter_id_mismatch` (3 of 3), `ledger_defines_nothing` (3 of 3),
`worktree_scoped_row` (2 of 2). `abs_path_outside_managed_roots`' 10 remain its own
legitimate subject.

The open question named when this was filed still stands and should be answered before
any of the five is touched: **read each check's repair path before scoping its report.**
`worktree_scoped_row`'s `fix=reseat_worktree` operates on catalog rows rather than on
files under a root, so it may legitimately be machine-wide. That agreement between
report and repair, not the row count, is what this bug is about.

`scan_premature_archive_citation` still needs its `archived`/`live` partition keyed
per-repo in the same change that scopes it — see § Evidence. Population measured empty
today, so it is latent; do not let a green reading stand in for the fix.
## Tests added

Three, all in `src/librarian/tools/doctor.rs`, and **mutation-verified rather than
merely green** — the standard `442d8b7c` set for this file:

- **`cited_prefix_reports_only_the_active_projects_citers`** — both directions in one
  test, using two separate prefixes so that "scopes nothing" and "scopes everything"
  fail differently. Deleting the scoping fails it, printing the leaked sibling-root
  violation.
- **`cited_prefix_definers_stay_corpus_wide_across_project_roots`** — the half that
  must NOT scope, and the reason the fix is not simply "filter by root". Scoping the
  definer pass fails it with the check asserting *"no `## HY-N — <title>` heading
  exists anywhere in the corpus"* while that heading sits in the sibling repo. This is
  the test that guards the design decision rather than the behaviour.
- **`a_mostly_foreign_prefix_is_scoped_out_and_keyed_outside_the_project`** — paths
  chosen so the alphabetically-first citer overall IS the in-project one, so the
  obvious `files[0]` keying shortcut fails it. Mutation 1's failure output independently
  confirmed this case occurs.

Citations clear both thresholds in every fixture, so none can pass for the unrelated
reason of sitting under the noise floor.

Gate at fix time: `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
clean; `cargo test` **4584 passed / 0 failed / 46 ignored**.
## Workarounds

Filter client-side. The awk pairing in § Reproduction reduces the report to the rows
this session can act on, and needs no filesystem access:

```
awk -F'"' '/"check":/{c=$4} /"path":/{if($4 !~ /^\//) print c, $4}' @tool_<id>
```

## Resume

One check down, five to go — and the next session should **not** start from the
"drops by exactly 33" line this file used to carry. That prediction was wrong, and
wrong in an instructive way: re-applying the thresholds to a smaller citer set also
drops prefixes that were only *partly* local, so the drop is ≥ the foreign count, not
equal to it. Measured: 48 → 14, a drop of 34 against 34 foreign rows, on a corpus that
had itself grown by one unowned prefix since § Symptom was written. Compare old and new
binaries **on the same corpus** — `git stash push -- src/librarian/tools/doctor.rs`,
`cargo rb`, run `./target/release/codescout doctor --json`, restore — or the corpus
moves under the measurement.

Use the CLI rather than the MCP tool for this, and know why: the MCP layer relativizes
path fields for display, so "absolute path = foreign row" is a valid discriminator on an
MCP response and **invalid** on CLI output, where every path is absolute. Discriminate
against the project root explicitly (`index(p, root) != 1`). The tell that the naive
version is wrong: `entry_dated_stale` and `entry_cited_from_outside_but_undeclared` are
known-scoped, so if they read anything other than 0 foreign, the discriminator is broken
rather than the checks. That control is what caught it here.

Next concrete action: `terminal_status_with_caveat` (13 foreign of 44, the largest
remainder). Read its repair path first — the four smaller checks are 100% foreign and
look trivially scopeable, which is exactly the shape that hides a check whose repair is
legitimately machine-wide.
## References

- `docs/issues/archive/2026-08-27-doctor-reports-other-workspaces-rows-as-violations.md`
  — the predecessor; fixed, live-verified, archived. Accurate for its scope.
- `docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md`
  — the earlier bug on the same check that tuned paging rather than population.
- `docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md`
  — where the biggest-contributing check came from.
- Ruling 17 — `src/librarian/tools/doctor.rs:229`.
- `reconnaissance-patterns:R-117` — a fix that names a population asserts it is
  non-empty; why the cross-repo hazard here is filed as latent rather than live.

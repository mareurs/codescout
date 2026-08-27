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

**Not started.** Two peer sessions are committing into `src/librarian/tools/doctor.rs`
in this shared checkout as of 2026-08-27 12:13; this file is a capture-on-notice
record, not a claim on the work.

The shape is already established twice and should be followed rather than reinvented
— `known_workspace_roots(ctx, conn)` exists and is the third caller's for free:

- Each leaking check takes the known roots and returns
  `(violations, scoped_by_project)`, reporting only rows under a managed root.
- **The metric stays global.** This is the Ruling 17 requirement and the part a
  naive "drop them" fix breaks silently.
- `scan_premature_archive_citation` needs the partition keyed per-repo as well as
  the reported population scoped — see § Evidence. Scoping the report alone leaves
  the cross-repo false-negative in place.

An open question worth deciding before writing code, not after: whether
`worktree_scoped_row` should scope at all. Its repair (`fix=reseat_worktree`)
operates on catalog rows rather than on files under a root, so it may legitimately
be machine-wide. Read the repair before scoping the report — the two must agree,
and *that agreement*, not the row count, is what this bug is about.

## Tests added

None — nothing is fixed yet. The discriminating test already exists as a model:
`a_row_under_a_known_workspace_root_is_scoped_out_but_still_counted` in
`src/librarian/tools/doctor.rs`, which `442d8b7c` mutation-verified rather than
merely running green.

## Workarounds

Filter client-side. The awk pairing in § Reproduction reduces the report to the rows
this session can act on, and needs no filesystem access:

```
awk -F'"' '/"check":/{c=$4} /"path":/{if($4 !~ /^\//) print c, $4}' @tool_<id>
```

## Resume

Decide first whether this is worth a commit at all — 56 rows of noise in a manual,
read-only scan is real but low-severity, and `doctor.rs` currently has two other
sessions writing to it.

If yes: start from `known_workspace_roots` (`src/librarian/tools/doctor.rs:1183`) and
`scan_artifact_paths`' `(violations, scoped_by_project)` return shape, apply it to
`scan_cited_prefix_with_no_definer` first (33 of the 56 rows, single largest win),
and re-run the § Reproduction pairing to confirm the count drops by exactly 33 with
the global metric unchanged.

Do **not** scope `scan_premature_archive_citation` by the reported population alone —
its `archived`/`live` partition is basename-keyed and must be split per-repo in the
same change, or the fix leaves a cross-repo false negative behind a check whose doc
comment promises it is wrong in every world.

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


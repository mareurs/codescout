---
id: 21e270ac21bf180f
kind: bug
status: fixed
title: 'BUG: doctor''s report is still 52% other repos'' rows — Ruling 17 reached two check families, six others still report globally'
tags:
- librarian
- doctor
- scope
- reporting
closed: 2026-08-27
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

**Fixed in two commits.** All six checks named in § Symptom are resolved — five scoped,
one deliberately not — and the report carries **zero unintended foreign rows**.

### Round 1 — `cited_prefix_with_no_definer` (`5a7eb3e7`)

The largest single contributor, 33 of its own 47 findings foreign.

Ruling 17 lands on **both halves, in opposite directions**:

- **Definers stay corpus-wide.** Narrowing `known_prefixes` to the active project
  would make every prefix defined only in a sibling repo — including every cross-repo
  `<repo>:<TOKEN>` citation, which the resolver already and deliberately declines to
  turn into an edge — fire as *"no definer anywhere in the corpus"*. That manufactures
  false positives out of correct prose: the mirror of the false negatives Ruling 17
  names on the exposure side.
- **The firing decision is global too; only the report is filtered.** In-project citers
  must clear the same two thresholds, and the remainder is named in the violation's own
  message rather than hidden.
- Scoped-out prefixes are keyed by the first citer **outside** the project, not by
  `files[0]` — which is frequently the in-project one that fell below threshold.

Measured old binary vs new on the **same** corpus: 48 → 14, 34 scoped out, 48 − 14 = 34.

- SHA `5a7eb3e7596551128c27435a663f3eabce55c71e` on **`experiments`**
- patch-id `6706c5b65a881c9da49c8e27f02a40e36ebf8bde`

### Round 2 — the four row-grain checks (`748c44e8`)

**The repair-path read this file demanded up front is what set the shape** — the
discriminator is not *is the finding foreign* but **does this check's repair write
files**:

| check | repair | writes files? | outcome |
|---|---|---|---|
| `frontmatter_id_mismatch` | `repair_frontmatter_id` — refuses without a scope, already filters to one root | yes | **scoped**, to match its repair |
| `frontmatter_id_is_not_a_catalog_id` | none — deliberately excluded from that sweep | — | **scoped** |
| `ledger_defines_nothing` | none | — | **scoped** |
| `terminal_status_with_caveat` | none | — | **scoped** |
| `worktree_scoped_row` | `fix=reseat_worktree` — takes no root, filters by none | no, catalog rows only | **NOT scoped** |

`repair_frontmatter_id`'s report was the outlier, not its repair. `reseat_worktree`
reseats every unregistered worktree-scoped row in the catalog regardless of root, so
narrowing its report would understate what `confirm=true` is about to do — a worse
defect than the two rows of noise it removes. The omission is stated in code so it does
not read as an oversight.

Applied as **one filter over the finished violation list**, not four scoping blocks. The
two in-scan precedents scope inside their own loop because they need something that loop
computed (the known-elsewhere set; an indegree key); these four need only the row's own
path, so a single filter is less code and more auditable — the scoped set is a list you
read in one place. `entry_without_definition` is listed despite reporting zero foreign
rows today: it shares a scan with `ledger_defines_nothing`, and one scan whose two
outputs scope differently is a trap for the next reader.

| check | before | after |
|---|---:|---:|
| `frontmatter_id_is_not_a_catalog_id` | 3 | 0 |
| `frontmatter_id_mismatch` | 3 | 0 |
| `ledger_defines_nothing` | 3 | 0 |
| `terminal_status_with_caveat` | 44 | 31 (13 foreign → 0) |
| **`row_checks_scoped_by_project`** | | **22** |

3 + 3 + 3 + 13 = 22, matching the announced drop to the row.

- SHA `748c44e8224afe7e3118d08aaa06907ea266f7c7` on **`experiments`**
- patch-id `ed28cc0ee9d7ad880dcbf1951760832763415337`

### The aggregate this file's title named — re-measured before archiving

Required by `bug-fix-session-log:F-75`, which this bug's own predecessor generated:
a fix verified against its mechanism can leave its title's number untrue.

| | findings | foreign | unintended foreign |
|---|---:|---:|---:|
| at filing | 126 | 66 (52%) | 56 |
| now | **72** | 12 | **0** |

The residual 12 is stated rather than rounded away: **10** `abs_path_outside_managed_roots`,
which § Symptom excluded from the count at filing because a foreign path there *is* the
finding, and **2** `worktree_scoped_row`, now a documented decision rather than an
omission.

### Known gap, deliberately not swept

`snapshot_drift`, `params_behind_body` and `augmentation_declared_but_absent` are
row-grain and read-only, so they would be safe to scope — but they report **zero**
findings on this machine today, so scoping them would assert a population nobody has
measured (`reconnaissance-patterns:R-117`). They are left out on purpose. If one of them
starts firing across repos, add it to `SCOPED_ROW_CHECKS` — and read its repair path
first, because that is the step that changed the answer here.
## Tests added

Four, all in `src/librarian/tools/doctor.rs`, and **every one mutation-verified rather
than merely green** — the standard `442d8b7c` set for this file.

**Round 1:**

- **`cited_prefix_reports_only_the_active_projects_citers`** — both directions in one
  test, two separate prefixes, so "scopes nothing" and "scopes everything" fail
  differently. Deleting the scoping fails it, printing the leaked sibling-root violation.
- **`cited_prefix_definers_stay_corpus_wide_across_project_roots`** — the half that must
  NOT scope. Scoping the definer pass fails it with the check asserting *"no `## HY-N —
  <title>` heading exists anywhere in the corpus"* while that heading sits in the sibling
  repo. Guards the design decision, not the behaviour.
- **`a_mostly_foreign_prefix_is_scoped_out_and_keyed_outside_the_project`** — paths chosen
  so the alphabetically-first citer overall IS the in-project one, so the `files[0]`
  keying shortcut fails it.

**Round 2:**

- **`row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not`** — three
  rows, one `call()`. Adding `worktree_scoped_row` to `SCOPED_ROW_CHECKS` — the
  *finish-the-job* mutation a future reader will reach for, since that check looks
  identical to the four scoped ones — fails it with the worktree row appearing in the
  scoped-out map. Disabling the scoping fails it with the sibling-root row surviving.

Gate at fix time: `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
clean; `cargo test` **4589 passed / 0 failed / 46 ignored**.

Live-verified on the rebuilt binary via `./target/release/codescout doctor --json`,
which runs the freshly built bytes directly rather than through a long-lived MCP server.
That distinction is not pedantry here — see § Workarounds.
## Workarounds

None needed now. While the fix was unbuilt, filtering client-side worked:

```
awk -F'"' -v root="<project root>/" '/"check":/{c=$4} /"path":/{if(index($4,root)!=1) print c, $4}' <report>
```

**One live caveat that outlives the fix.** A long-running MCP server keeps serving the
binary it started with, so `librarian(action="doctor")` reports the OLD numbers until
that process is restarted — measured here at the moment of the round-1 fix: the CLI said
14 findings / 94 total while this session's own MCP said 48 / 128, same machine, same
corpus, same minute. Twelve of thirteen codescout servers on this host were running
deleted inodes at the time. Reconnect (`/mcp`) before trusting an MCP `doctor` reading
after any rebuild, or use the CLI. See
`docs/issues/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`.
## Resume

N/A — fixed, live-verified, archived.

Two measurement traps were hit while fixing this, both recorded because the next
session will hit them in the same order:

- **Comparing across corpora.** The 47/33 figure in § Symptom was an hour and five
  markdown commits old, and the corpus had gained an unowned prefix in between — that,
  not an accounting error, is the whole 33-vs-34 discrepancy. `git stash push --
  src/librarian/tools/doctor.rs`, `cargo rb`, measure, restore — or the corpus moves
  under the measurement. This file also used to predict *"the count drops by exactly
  33"*; it does not, because re-applying the thresholds to a smaller citer set also
  drops prefixes that were only partly local. The drop is ≥ the foreign count.
- **"Absolute path = foreign row"** is valid on an MCP response and **invalid** on CLI
  output, because relativization is an MCP display-time transform. The naive version
  reported 94/94 foreign. What caught it was a built-in control: `entry_dated_stale` and
  `entry_cited_from_outside_but_undeclared` are known-scoped, so their reading anything
  other than 0 foreign meant the discriminator was broken rather than the checks.
  Discriminate against the project root explicitly (`index(p, root) != 1`).
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

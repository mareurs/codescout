---
id: 33e740960f758b6d
kind: bug
status: fixed
title: 'BUG: doctor accepts a scope argument and never reads it — scope=all silently under-reports by 636 rows'
tags:
- librarian
- doctor
- schema-drift
- cluster/accepted-parameter-silently-dropped
closed: 2026-09-10
opened: 2026-09-09
owner: marius
related:
- a00b99a98e5401dd
severity: medium
unverified: 'CLEARED 2026-09-10. Was: "Typed/validated/echoed only (26b60af8). The population selector still ignores `scope` -- the headline 636-row under-report is LIVE." That claim is stale: 14 further commits (fe7b6658..566dd859, full SHA+patch-id table in Sec Fix) gave every scan a shared DoctorScope unit, so the population selector now reads scope. Re-measured 2026-09-10 via the worktree''s own built CLI binary (./target/debug/codescout doctor, HEAD 566dd859, clean tree): catalog_health.outside_roots_total = 2041 (exact, unconditional at every scope per Ruling 17) -- not a like-for-like replacement for the original 636 (which summed 4 categories: 462 outside-roots + 113 entry-validity + 21 cited-prefix + 40 row-grain); the fuller 4-category analog on this catalog at this instant is 2041 + 116 + 21 + 433 = 2611. Remaining precondition is the merge from doctor-per-project-isolation to experiments (14 of 15 fix-cohort commits are not yet there), not any code -- do NOT archive until that merge lands.'
---

> **Cluster:** `cluster/accepted-parameter-silently-dropped` (`IC-15`, n=20 before this
> file). Second instance *on the `scope` param specifically* — the first was
> `audit_doc_refs` (`docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md`,
> artifact `a00b99a98e5401dd`, fixed 2026-07-06 by shipping the **reject** path).

## Summary

The shared `librarian` tool schema declares `scope` with `"default": "project"`, and
`doctor::call` never reads it. A caller passing `scope="all"` receives a report scoped
the same as `scope="project"` — no error, no warning, and a `summary.total` that
under-reports the true machine-wide population by **636 rows** (~79%). The report even
*names* the rows it dropped, in `catalog_health`, while the param that should have
retrieved them is discarded.

## Symptom (Effect)

Four readings against the same tree, taken within minutes:

```
librarian(action="doctor")                     → summary.total = 170
librarian(action="doctor", scope="project")    → summary.total = 169
librarian(action="doctor", scope="umbrella")   → summary.total = 169
librarian(action="doctor", scope="all")        → summary.total = 169
```

`by_check` is identical across the three scoped runs, and the `umbrella` and `all` runs
returned a **byte-identical response buffer** (`144549` bytes, same `output_id`). The
170→169 step is corpus drift, not the param: `informational` went 3→2 as a live
`claim_held_by_live_session` expired mid-session.

No `RecoverableError`, no `scope_fallback`, and no `scope` block in the response — unlike
`doc(action="find")`, which echoes `"scope": {"applied": "project", …}`.

## Reproduction

```
git rev-parse HEAD          # 4d928f2d (experiments); src/librarian/tools/doctor.rs clean
cargo rb && /mcp            # live-MCP release build, then reconnect
librarian(action="doctor", scope="all",     limit=1)   # read summary.total
librarian(action="doctor", scope="project", limit=1)   # read summary.total
```

**The `all` reading is the load-bearing one and must not be dropped from this
reproduction.** Comparing *absent* against `scope="project"` is the single comparison
that cannot discriminate: those two agree just as readily when the param **works** and
merely defaults to `project`. Only a value obliged to widen can separate "ignored" from
"honoured and redundant". Recorded as `bug-fix-session-log:F-125`; the correction came
from a peer session (sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`), not from the
author.

The expected value for a working `all` is derivable from the same response:
`catalog_health.hint` reports 462 outside-roots + 113 entry-validity + 21 cited-prefix +
40 row-grain = **636** rows scoped out, so `all` owed ≈ 805.

## Environment

Linux 7.2.3-zen1-3-zen · branch `experiments` @ `4d928f2d` · MCP stdio transport ·
project `codescout` · profile `~/.claude-sdd` · catalog spanning 112+ project roots and
10 managed roots.

## Root cause

`doctor::call` (`src/librarian/tools/doctor.rs:326`) reads exactly seven arguments
through untyped `args.get(...)` accessors — `fix`, `confirm`, `old_root`, `root`,
`new_root` (`:329-350`), `limit` and `offset` (`:757-766`). `scope` is not among them,
so the value is deserialized by nothing and discarded.

It is *declared* for the whole tool: `src/librarian/tools/librarian.rs:64-68` defines one
flat JSON schema shared by all 11 actions, with `scope` carrying
`"default": "project"` and a **prose** enumeration of which actions honour it — AT THE
TIME this bug was filed, that prose read `context/reindex/workspace_state_at/link_scan`,
plus `audit_doc_refs` which rejects non-`project`. `doctor` was absent from that prose —
correctly, for that snapshot — but prose was never a constraint, so the schema still
accepted the param for `doctor` and the handler still ignored it.

**Stale as of the fix (2026-09-09 whole-branch review round 2, Important 4): the prose
now reads `context/reindex/workspace_state_at/link_scan/doctor` — `doctor` was added once
the handler actually started reading the param (see the Fix section's cohort).** The
paragraph above is left in its original, now-superseded form because it is the
reproduction's own diagnosis of the bug as it stood, not a live claim about current
schema text; read `src/librarian/tools/librarian.rs`'s own `scope` property for the
current prose rather than this file.

The tool's own param probe predicts this in writing. `librarian.rs:234-239`:

> `// Both are doctor's, both read through untyped accessors, so no value is`
> `// ill-typed for them. Admissions of blindness, not passes … A typed Args for`
> `// doctor would let the probe reach them.`

That comment is about `fix` and `offset`, and it is an honest admission for those two.
It does **not** cover `scope`, and the reason is a second defect one level up.

`param_probe::sweep` resolves which action a schema key belongs to with
`desc.split(':').next().and_then(|l| l.split('/').next())`
(`src/tools/param_probe.rs:108`) — **the first slash token, with no loop over the rest.**
`scope`'s description begins `context/reindex/workspace_state_at/link_scan:`, so the probe
checks it as `context:scope` and for no other action. `doctor` is absent from that label
entirely, so the pair `doctor:scope` was never in the probed set — it is not exempted, and
it is not admitted in `accepts_any_json` either. `fix` and `offset` are declared
unreachable; `scope` was invisible in both directions.

*measured 2026-09-09: `cargo test --workspace
every_action_labelled_schema_key_is_honored_by_that_action` → **green** (3 passed), while
`doctor` demonstrably discards `scope`. Had the probe reached `doctor:scope` it would have
found `base == probed` — an ill-typed `scope` changes nothing for a handler that never
reads it — and reported the key unhonored, reddening the test. Green therefore **proves**
the pair is unchecked; it is not weak evidence about it.*

So the accurate form is: the guard that would have caught this exists, `doctor` reads its
args through untyped accessors, **and the guard's selector never reached the pair anyway.**
A typed `Args` alone is necessary and not sufficient — adding `doctor` to `scope`'s label
leaves it last in the slash list, where the selector still will not see it. Raised by a
peer session (sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`) as a binary — probe defect
or missing admission — and it is neither; the third option is the selector. Tracked as
`docs/issues/archive/2026-09-09-param-probe-checks-one-action-per-shared-key.md` — **fixed
and archived 2026-09-09** (`80c4fd1e`, patch-id `3f662b6154f5c76751495977cbab12b650518e8f`).
That fix falsifies the sentence just above: with the selector now looping every slash
token, a `doctor` token sitting **last** in `scope`'s label would be swept. So adding
`doctor` to that label is live work rather than a dead end — and the probe should then red
on this very bug, because `doctor` still discards `scope` at the population selector.

*measured 2026-09-09: the four `summary.total` readings above, plus a `grep` over
`doctor::call` for `scope` returning no match. Mechanism read from source **and**
observed at runtime; neither alone is sufficient here.*

## Evidence

### The dropped rows are named in the same response that discards the param

`catalog_health.hint`, from the `scope="all"` run:

```
462 outside-managed-roots row(s) across 112 project root(s) were scoped OUT of this
report because they belong to a workspace this machine knows about … 113 entry-validity
row(s) … scoped out of this report because they belong to 7 other project root(s) … 21
cited_prefix_with_no_definer finding(s) across 10 other project root(s) were scoped OUT
… 40 row-grain finding(s) … across 10 other project root(s) were scoped OUT
```

Every one of those sentences describes a decision `scope="all"` exists to reverse.

### `doctor` has five scoping mechanisms and no scope parameter

Ruling 17 (*the metric stays global, the worklist is the active developer's*) is
implemented five separate ways, none of them caller-controllable:

| mechanism | site | reports the drop? |
|---|---|---|
| `known_workspace_roots` + in-loop filter | `doctor.rs:1801`, applied `:1762` | yes |
| four per-scan `ctx` filters + `scoped_map` | `:3164`, `:3314`, `:3439`, `:3544` | yes |
| same shape, own map | `:3969` | yes |
| `retain` over a 7-string `SCOPED_ROW_CHECKS` list | `:551-587` | yes |
| silent inline `containing_root(cp.git_root, …)` | `:5347` and 5 siblings | **no** |

The last row is a second, smaller defect in the same area: those scans scope correctly
and publish no count, so a reader cannot tell whether `terminal_status_without_fix_anchor:
9` is repo-local or machine-wide. Not split into its own file — same fix, same commit.

## Hypotheses tried

1. **Hypothesis:** `scope` is honoured and `project` is simply the default, making the
   first two readings agree legitimately.
   **Test:** take a reading at a value obliged to widen — `scope="all"`, then
   `scope="umbrella"`.
   **Verdict:** rejected. Both returned `169` with `by_check` identical and a
   byte-identical buffer, against a derived expectation of ≈805.
   **Evidence link:** § Symptom.

2. **Hypothesis:** the value is read and then deliberately discarded, e.g. `doctor`
   normalising every scope to `project` on purpose.
   **Test:** `grep` `doctor::call` for `scope`.
   **Verdict:** rejected — no read at all. Distinguishing this mattered: a deliberate
   normalisation would make the fix a *rejection* (the `audit_doc_refs` remedy), not an
   implementation.
   **Evidence link:** § Root cause.

## Fix

> **FIXED, and LANDED on `experiments` 2026-09-10** at `4ff09107` (fast-forward). All
> nineteen non-merge commits below were verified as ancestors of `experiments`
> individually — `git merge-base --is-ancestor <sha> experiments` for each, 19 of 19 —
> rather than inferred from the branch tip being reachable, which would be one claim
> standing in for nineteen. `26b60af8` (patch-id
> `39641840397a72f257450e7d36b72e197a1c67bf`, **on experiments**) typed `doctor`'s args, so
> `scope` is deserialised, an unknown value is refused rather than ignored, and the
> applied scope is echoed in the response. Two regression tests cover that much:
> `an_unknown_scope_value_is_refused_rather_than_ignored` and
> `the_applied_scope_is_echoed_in_the_response`.
>
> **The population selector reads it now.** Fourteen further commits (listed in full
> below) gave every scan a shared `DoctorScope` unit that six ad-hoc scoping mechanisms
> collapsed onto, threaded `fix=reseat_worktree`'s repair and its report through that same
> unit so the two cannot disagree, added a cites-based relevance exemption (`artifact_link`
> and `entry_cite`) that admits a foreign finding only when a local artifact cites it
> (narrowed to umbrella siblings, matching the user's stated ceiling), and made
> `catalog_health.outside_roots_total` exact and unconditional at every scope. `scope="all"`
> no longer silently re-labels a project-scoped report as widened.
>
> **And the shape changed in a direction worth naming when `26b60af8` alone had landed —
> kept here because the wrong turn is instructive.** Before that commit, the param was
> discarded in silence. For the window between `26b60af8` and the rest of this cohort, the
> response *asserted* `scope: all` over a population that was still project-scoped, which a
> caller could not distinguish from a correctly widened one — a state strictly worse than
> the original silence, which at least under-claimed. That reads at first like `IC-3`
> (declaration is not execution), and it was briefly retagged so — wrongly. **The class
> stayed `IC-15`, on this ledger's own precedent:** IC-15 moved
> `cli-artifact-drops-time-scope-and-extra` out to IC-3 *because no CLI flag existed, so
> nothing was accepted*, and states its own claim as "a value the caller passed and the
> system took, then did not use". `scope` was accepted — parsed, type-checked and
> enum-validated — and, for that window, not used, which is that sentence exactly. What
> `26b60af8` changed on its own was the **feedback**, not the class: acceptance became
> explicit rather than silent, the same wrinkle IC-15 already records for
> `read-only-true-is-inert-at-every-root`, whose echoed `read_only: false` put a
> discriminator in a field nobody reads. (A fix for a sibling class can make an IC-15
> member *look* like an IC-3 one, since every stage of wiring can be present except the one
> that reads the value. The discriminator is whether the parameter is accepted at all —
> not how much machinery sits between acceptance and the drop.) The rest of the cohort is
> what closes that window for good.

**Shipped the implement path, not the reject path — the opposite of the `audit_doc_refs`
precedent, and the difference was load-bearing.** That bug rejected `repo`/`umbrella`
because `audit_doc_refs` walks the filesystem and widening was real feature work. Here the
widening machinery already existed five times over; what was missing was only the
caller's control over it. Rejecting non-`project` would have left the scoped-out rows
permanently unreachable *and* left five mechanisms uncollapsed.

**One correction to the plan actually implemented: the SQL-splice route (originally
planned item 3 below, citing `bug-fix-session-log:W-117` as proof it was reusable) was
abandoned, not shipped.** Task 2's implementer proved with a `git stash` control that a
scoped-out tally requires observing the rows the SQL layer would have excluded before they
ever reach the handler — so splicing `apply_scope`'s `FilterNode` into each raw
`conn.prepare` scan and Ruling 17 (the metric stays global, the worklist narrows) are
mutually exclusive by construction. `W-117`'s specific instance is recorded falsified in
`bug-fix-session-log:W-117`; its pattern (scout whether the abstraction is *consumable*,
not merely present) is preserved as sound. What shipped instead:

1. `doctor` got a typed `Args` struct with `#[serde(default)] scope: Option<Scope>`,
   bringing `fix`/`limit`/`offset` under the `librarian.rs` param probe and letting the
   `accepts_any_json` exemption be deleted. (`26b60af8`)
2. `scope::resolve_scope(requested, current, UmbrellaPolicy::Require, Scope::Project)` —
   `Require` because `doctor` is a search-shaped surface like `find`, not an orientation
   surface like `context`. (`26b60af8`)
3. A new `DoctorScope` unit (`src/librarian/tools/doctor/scope.rs`) applies scope by
   **admission**, not by SQL: every scan calls `doctor_scope.admit(check, id, abs_path)`
   per candidate row, folding scoped-out rows into a published tally instead of excluding
   them before they can be counted. Six ad-hoc scoping mechanisms (`known_workspace_roots`,
   four per-scan `ctx` filters, the `SCOPED_ROW_CHECKS` retain-list, and the silent inline
   `containing_root` checks) collapsed onto this one unit. (`fe7b6658` through `cfacf2fc`)
4. `fix=reseat_worktree`'s repair takes the same resolved `DoctorScope` as the report, so
   repair and report share one scope and cannot disagree. (`fe1c41e3`)
5. A cites-based relevance exemption (`artifact_link` and `entry_cite`) admits a foreign
   finding when a local artifact cites it, narrowed to umbrella members so the user's
   ceiling ("at most umbrella siblings, only if it affects the current project") holds.
   (`bd691d24`, `5e78ab59`)
6. The applied scope and any `scope_fallback` are echoed in the response, and
   `catalog_health.outside_roots_total` is an exact, unconditional-at-every-scope sum —
   the metric stays global (Ruling 17) even as the collapsed display narrows below
   `scope=all`. (`566dd859`)

**Fix cohort — SHA and patch-id for every commit** (`git show <sha> | git patch-id
--stable`; derive independently with `git log --format=%H experiments..doctor-per-project-isolation`
rather than trusting this table, which is a lower bound written at one instant —
2026-09-10):

| # | SHA | patch-id | subject | on `experiments`? |
|---|---|---|---|---|
| 1 | `26b60af8` | `39641840397a72f257450e7d36b72e197a1c67bf` | type doctor's args so a declared scope cannot be discarded | **yes** |
| 2 | `fe7b6658` | `acc810f2b9973b1011dfc5796d6e288f423866ef` | doctor scoping unit -- admit-based narrowing, not SQL-splice | **yes** |
| 3 | `67e9804e` | `ee76a6d47759c68ce481aff2028467c4989dde88` | restore Ruling 17 for scope-refused outside-roots rows | **yes** |
| 4 | `0334dd88` | `5f8f3190812e0a7a17d27a05db554b087dbdb95c` | fix Round-2 findings on DoctorScope -- false hint text, fold ordering, admit() validation | **yes** |
| 5 | `c1d8cd51` | `0a3a9c9e77b0db5fa66af06f2a3fc6ef8d04300f` | scope five more checks through DoctorScope | **yes** |
| 6 | `1f0cd9c1` | `1aeebf7bf5e6229c3206265ffd78576188b2c23e` | address round-3 review of doctor Task 3 DoctorScope conversion | **yes** |
| 7 | `55c77f25` | `299e7e8f9b97c999e36752d05ce7533ef17af03f` | retire SCOPED_ROW_CHECKS; row-grain checks admit() directly | **yes** |
| 8 | `15d141eb` | `34abe38ad6229550fd839a58016b1eb39a6c6060` | address round-2 review of Task 4 (row-grain scoping) | **yes** |
| 9 | `8cef7950` | `ff7d6e70c7d424784dc44fed3ba7d407c1f6579d` | close round-2 review gaps in row-grain scope coverage | **yes** |
| 10 | `cfacf2fc` | `294ad2cda6c1ede95e5bcb35266c86f2c86ab0a6` | give six silent-scoping scans a published DoctorScope tally | **yes** |
| 11 | `fe1c41e3` | `7aae9ca5e9a15017d23ae6e17abaf4a82d3ea3c8` | scope reseat_worktree's repair, then its report | **yes** |
| 12 | `bd691d24` | `0b7219f8cd6f9cc87ad6934611f52ad5b630ebd7` | Task 7 -- relevance exemption for cross-root cites, with its own denominator | **yes** |
| 13 | `5e78ab59` | `bd596e0a92e3e0cd32b4ec3d4725ea76a43fe2ab` | Task 7 review round 1 -- five semantic fixes to the relevance exemption | **yes** |
| 14 | `abc61b68` | `1e20d432272c3a89ce428ddd5953a1d50cb06075` | DoctorScope::new takes conn directly, not a second internal lock | **yes** |
| 15 | `566dd859` | `ad17d2428dee5f43bb4fe857ee3221478185d69d` | collapse the outside-roots inventory below scope=all | **yes** |
| 16 | `240b7531` | `66434dccabaa98aba52b7b194bf1674064b19953` | Task 9 -- sweep doctor's scope claims, name the Scope::All trap in comments | **yes** |
| 17 | `6fc18304` | `4cab2565e570afe3c2316a19523eb30340b60f70` | whole-branch review round 2 -- three Criticals, two Importants, one Minor | **yes** |
| 18 | `2bfb2e15` | `7facca854a76a76caac2667b39b19357eebee51e` | correct three dry-run claims, verify M1/M2 exemption comments | **yes** |
| 19 | `ccfdbf4a` | `54522a04cdf684cdcc77e7cf2e88eaf628b388e9` | the ProbeRow the doctor.rs cap-class annotation requires | **yes** |

**Row #12 (`bd691d24`) is not in the plan's own cohort list and was found only by
re-deriving `git log --format=%H experiments..HEAD` rather than trusting it** — the plan
named 14 total entries (13 SHAs plus `26b60af8`) where 15 exist. This is exactly the kind
of drift the derivation instruction above warns about.

**The table above ran to 15 rows until 2026-09-10 and the real figure was 19** — rows
16-19 were found by re-deriving `git log --no-merges 0f060fe3..4ff09107 ^7a097d68` rather
than by trusting it, which is the second time this one table has been short (row #12 was
the first). The derivation instruction in the caption is not decoration; both misses were
found by following it and neither would have been found by reading.

**How the merge landed, and the three merge commits it took.** Integration was a MERGE
rather than a rebase, deliberately: a rebase rewrites every SHA, and this table's whole
point is that the SHAs resolve. `experiments` had moved 133 commits ahead of `0f060fe3`
by then and moved twice more mid-flight, so three rounds were needed —
`8cf67de0` (content conflicts in `doctor.rs` and `server.rs`), `4485eeb0` (the
parameter-alias collapse), `4ff09107` (docs). `experiments` was then fast-forwarded, so
its history is linear through them.

**No patch-id is recorded for those three, and the reason is sharper than CLAUDE.md's
rule states.** The rule says a merge commit has none because `git show` emits no diff.
Measured here: true of `4485eeb0` (a clean merge — 457 bytes, no diff, empty patch-id),
and **false of `8cf67de0`**, whose conflict resolution makes `git show` emit a 32,634-byte
combined diff from which `git patch-id --stable` returns a plausible
`9817e7c6…`. That value hashes only the resolution hunks, not the change the merge
delivers, so recording it would be worse than recording nothing — the documented failure
mode is an empty field you notice, and the actual one for a conflicted merge is a
well-formed wrong answer. Cite the constituent commits, which is what this table is.

**The one conflict worth a note for whoever reads `doctor.rs` next.** `experiments` grew a
parallel `SCOPED_ROW_CHECKS` array (`b057cc6d`) solving the same problem from a
`retain()` filter rather than an `admit()` gate, with the same first 11 members in the
same order. It was removed in favour of `ROW_GRAIN_SCOPED_CHECKS` (22 members, verified a
strict superset member by member), but **two things ported forward rather than being
dropped**: its per-member measurements (`params_behind_body` firing 3 times with 2
foreign, `params_status_drift` 3 with 1, and which members were added on a sighting versus
swept in on structure) and its two tests, one of which — `the_scoped_row_check_hint_names_
every_check_it_scoped` — asserts on the real report's hint string with a doc comment
recording that the generator-versus-itself version of it "passed instantly against the
live drift". Nothing of that branch's was lost; the merge kept the better half of both.
## Tests added

None yet. Planned, mirroring the precedent's four
(`audit_doc_refs`: `scope_repo_is_rejected` / `scope_umbrella_is_rejected` /
`scope_project_is_accepted` / `scope_absent_is_accepted`):

- `doctor_scope_all_reports_more_than_scope_project` — **the discriminating test**, and
  the one this bug's own reproduction shows is easy to omit. An equality assertion between
  `absent` and `project` is monotone under the defect and would have stayed green
  throughout.
- `doctor_scope_absent_defaults_to_project`
- `doctor_scope_is_echoed_in_the_response`
- `doctor_scope_fallback_is_set_when_no_project_is_active`

## Workarounds

At the default scope, none needed — the report is already project-scoped by the five
internal mechanisms, so **today's output is correct for `scope="project"`**; only
widening is unavailable.

To reach the suppressed population meanwhile, read
`catalog_health.outside_roots_scoped_by_project`,
`.entry_validity_scoped_by_project`, `.cited_prefix_scoped_by_project` and
`.row_checks_scoped_by_project` — they carry per-root counts for all 636 rows, though not
the rows themselves. There is no way to obtain the individual foreign findings.

## Resume

Add the typed `Args` struct to `src/librarian/tools/doctor.rs` (currently `call` at `:326`
takes bare `Value`) and write
`doctor_scope_all_reports_more_than_scope_project` FIRST, asserting a strict inequality —
run it and watch it fail before touching `call`, because the equality form of that
assertion passes against the bug. Then thread `resolve_scope`/`apply_scope`, splicing
`filter::compile`'s `SqlFragment` into `scan_artifact_paths` (`:1714`) as the first
convert, since it is the scan the existing `known_workspace_roots` filter already sits
inside. Delete `accepts_any_json: &["fix", "offset"]` (`src/librarian/tools/librarian.rs:239`)
in the same change and confirm the param probe goes green rather than assuming it.

Note `src/cli/doctor.rs:57` passes `Value::Object(Map::new())` — an empty args map — so the
CLI reaches none of this; its doc comment at `:5` still claims *"no doctor-specific args
yet because the scanner takes no input"*, which is false of `limit`/`offset`/`fix`/`root`.
Filed separately.

## References

- `src/librarian/tools/doctor.rs:326` (`call`), `:551-587` (`SCOPED_ROW_CHECKS`), `:1714`
  (`scan_artifact_paths`), `:1801` (`known_workspace_roots`), `:5342-5347`
  (a silent-scoping scan)
- `src/librarian/tools/librarian.rs:64-68` (the flat `scope` schema), `:234-239` (the
  param-probe exemption, which covers `fix`/`offset` and NOT `scope`)
- `src/tools/param_probe.rs:108` (the first-slash-token selector that never reached
  `doctor:scope`) — its own bug file:
  `docs/issues/archive/2026-09-09-param-probe-checks-one-action-per-shared-key.md`
- `src/librarian/tools/scope.rs` (`Scope`, `UmbrellaPolicy`, `resolve_scope`, `apply_scope`)
- `src/librarian/filter.rs:92` (`compile`), `src/librarian/catalog/find.rs:20-26` (the splice)
- Precedent, same class, same param, different action:
  `docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md`
- Prior rounds of doctor project-scoping, both archived and both accurate for their scope:
  `docs/issues/archive/2026-08-27-doctor-reports-other-workspaces-rows-as-violations.md`,
  `docs/issues/archive/2026-08-27-doctor-still-reports-52pct-foreign-rows-via-six-other-checks.md`
- `bug-fix-session-log:F-125` (the non-discriminating proof), `:W-117` (the SQL-splice scout)

---
id: d4b61746950b86b7
kind: bug
status: open
title: 'BUG: doctor accepts a scope argument and never reads it — scope=all silently under-reports by 636 rows'
tags:
- librarian
- doctor
- schema-drift
- cluster/accepted-parameter-silently-dropped
- cluster/declared-not-wired
closed: null
opened: 2026-09-09
owner: marius
related:
- a00b99a98e5401dd
severity: medium
unverified: Typed/validated/echoed only (26b60af8). The population selector still ignores `scope` — the headline 636-row under-report is LIVE. Verified at the bytes 2026-09-09.
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
`"default": "project"` and a **prose** enumeration of which actions honour it
(`context/reindex/workspace_state_at/link_scan`, plus `audit_doc_refs` which rejects
non-`project`). `doctor` is absent from that prose — correctly — but prose is not a
constraint, so the schema still accepts the param for `doctor` and the handler still
ignores it.

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

> **PARTIAL as of 2026-09-09 — do NOT archive; the headline symptom is still live.**
> `26b60af8` (patch-id `39641840397a72f257450e7d36b72e197a1c67bf`, **experiments**) typed
> `doctor`'s args, so `scope` is now deserialised, an unknown value is refused rather
> than ignored, and the applied scope is echoed in the response. Two regression tests
> cover that much: `an_unknown_scope_value_is_refused_rather_than_ignored` and
> `the_applied_scope_is_echoed_in_the_response`.
>
> **The population selector still does not read it.** Verified at the bytes 2026-09-09:
> `effective_scope` has exactly two uses in `src/librarian/tools/doctor.rs` — the
> `resolve_scope` binding, and echoing itself back in the `"scope"` block of the
> response. Every scan takes `roots`, and `roots = super::managed_roots(ctx)`, derived
> from the context alone. So `scope="all"` still returns a project-scoped report and the
> 636 dropped rows are still dropped.
>
> **And the shape changed in a direction worth naming.** Before, the param was discarded
> in silence. Now the response *asserts* `scope: all` over a population that is
> project-scoped — the report states a scope it did not apply, which a caller has no way
> to distinguish from a correctly widened one. That is the same class one layer out: the
> declaration is well-formed and validated, and nothing wires it to the selector
> (`cluster/declared-not-wired`).
>
> Remaining work is the original Fix below: thread `effective_scope` into `managed_roots`
> (or into the row filter) so the scanned population actually widens. A regression test
> must assert a **row-count difference** between `scope="project"` and `scope="all"` over
> a two-root fixture — asserting the echoed value cannot fail against this bug, since the
> echo is precisely the half that already works.

**Ship the implement path, not the reject path — the opposite of the `audit_doc_refs`
precedent, and the difference is load-bearing.** That bug rejected `repo`/`umbrella`
because `audit_doc_refs` walks the filesystem and widening was real feature work. Here
the widening machinery already exists five times over; what is missing is only the
caller's control over it. Rejecting non-`project` would leave the 636 scoped-out rows
permanently unreachable *and* leave five mechanisms uncollapsed.

Planned, in one change:

1. Give `doctor` a typed `Args` struct with `#[serde(default)] scope: Option<Scope>`,
   which also brings `fix`/`limit`/`offset` under the `librarian.rs` param probe and lets
   the `accepts_any_json` exemption at `:239` be deleted.
2. Resolve via `scope::resolve_scope(requested, current, UmbrellaPolicy::Require,
   Scope::Project)` — `default = Project` per `scope.rs:11`, `Require` because `doctor` is
   a search-shaped surface like `find`, not an orientation surface like `context`.
3. Apply at the **SQL layer**, not as a sixth post-hoc filter: `apply_scope` yields a
   `FilterNode`, `filter::compile` is `pub` (`src/librarian/filter.rs:92`) and returns
   `SqlFragment { sql, params }`, which each raw `conn.prepare` scan splices exactly as
   `cat_find::find` does (`src/librarian/catalog/find.rs:20-26`). Verified reusable —
   `bug-fix-session-log:W-117`. This also removes the per-repo `read_to_string` cost that
   post-hoc scoping cannot.
4. Echo the applied scope (`ScopeApplied::to_json`) and any `scope_fallback` in the
   response, so a widened-because-no-active-project result explains itself.
5. Re-grain the 13 `cp.git_root` sites to `Scope::Project` (`cp.abs_path` + `main_root`
   worktree overlay) so "project" means here what it means in `doc(action="find")`.
6. Preserve Ruling 17: the `*_scoped_by_project` metrics stay global at every scope.

**SHA:** N/A — not yet fixed.
**patch-id:** N/A — not yet fixed.

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

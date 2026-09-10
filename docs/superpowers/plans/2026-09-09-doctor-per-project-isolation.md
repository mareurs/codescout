---
kind: plan
status: draft
title: Doctor Per-Project Isolation Implementation Plan
owners:
- marius
tags:
- librarian
- doctor
- scope
- project-isolation
topic: doctor per-project isolation
---

# Doctor Per-Project Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `librarian(action="doctor")` report only the active project's findings by default, with caller-controlled widening, by replacing five ad-hoc scoping mechanisms with one applied at the SQL layer.

**Architecture:** `doctor::call` gains a typed `Args` struct with `scope: Option<Scope>`, resolved through the existing `scope::resolve_scope`. A new `DoctorScope` helper wraps `apply_scope`'s `FilterNode` compiled to a `SqlFragment` (for scans that query) plus a lexical path predicate (for scans that already hold a path), so every scan narrows the same way and the `*_scoped_by_project` metrics stay global — Ruling 17 preserved. Foreign findings surface only when a local artifact cites them, and the report publishes how many `cites` edges that rule was computed over, because the answer is currently **zero**.

**Tech Stack:** Rust · `rusqlite` · `serde` · `tokio` (tests) · the librarian catalog at `~/.local/share/librarian/catalog.db`

**Spec:** `docs/issues/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md` (artifact `d4b61746950b86b7`) § *Fix* and § *Tests added*. Sequenced-after sibling: `docs/issues/archive/2026-09-09-cli-doctor-passes-an-empty-args-map-so-no-fix-or-paging-is-reachable.md` — **out of scope for this plan**, deliberately, because Task 1 changes the projection its fix would target. **FIXED and archived 2026-09-09** at `953c98f3` (patch-id `8de7522768dd6dacacd293eae5d881442470422a`), in the prescribed order: the typed `Args` landed first (`26b60af8`), the wrapper second. Note its id changed on archiving — the old `a06de4dfc30c2e8d` no longer resolves.

## Global Constraints

- **The gate is four commands in a load-bearing order,** run before completing any task: `cargo fmt`, `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`, `cargo test --workspace --no-default-features`, `cargo test --workspace`. **Chain the two test lanes with `;`, never `&&`** — read the exit codes.
- **The lean lane is VACUOUS for librarian code.** `--no-default-features` switches the librarian off, so it runs **zero** `librarian::` tests. Never read `LEAN exit=0` as evidence about work in this plan. Read your own test names out of the **default** lane: `grep -E '^test librarian::tools::doctor::tests::' <output>`.
- **Announce before running the gate.** Several sessions share this checkout and arm deliberate reds; a red you did not cause is likely theirs. `run_command` attaches `scripts/attribute-red.py` output on non-zero exit — read the `wip_authors` line. Native `Bash` bypasses this, so on a `Bash` gate the silence means nothing.
- **All work on `experiments`.** `master` is protected. Never commit in-progress work to `master`.
- **Ruling 17 is invariant:** the reported worklist narrows, the `*_scoped_by_project` and `outside_roots_by_project` metrics stay global at every scope. A metric that shrinks when the worklist does is the false negative this plan must not introduce.
- **Every commit message ends with:** `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`
- **Cite `F-125` / `W-117`** (`docs/trackers/bug-fix-session-log.md`) in commit messages that close part of this, and the bug id `d4b61746950b86b7`.
- **Wire strings are public vocabulary.** Check names (`"terminal_status_with_caveat"`, …) are pinned beside their enum variants on purpose and named in prose across `docs/`. Never derive one from an identifier; never rename one in this plan.

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `src/librarian/tools/doctor/scope.rs` | **New.** `DoctorScope` — the single scoping unit: holds the resolved `Scope`, the compiled `SqlFragment`, the lexical path predicate, and the scoped-out tally. | Create |
| `src/librarian/tools/doctor.rs` | `Args`, `call`'s orchestration, the ~30 scans. Loses five bespoke scoping blocks. | Modify |
| `src/librarian/tools/librarian.rs` | Flat tool schema; `scope` description; param-probe `accepts_any_json` exemption. | Modify `:64-68`, `:239` |
| `src/librarian/tools/scope.rs` | `Scope`, `resolve_scope`, `apply_scope`, `ScopeApplied`. | **Read-only** — reused unchanged |
| `src/librarian/filter.rs` | `compile` → `SqlFragment`. | **Read-only** — reused unchanged |

`doctor.rs` is ~15,500 lines and already unwieldy; extracting scoping into `doctor/scope.rs` follows the precedent of `link_scan/` and `audit_doc_refs/`, which are directories rather than single files. Do **not** attempt a broader split of `doctor.rs` in this plan.

---

### Task 1: Typed `Args` — accept, validate and echo `scope` without changing which rows are reported

Shippable alone: after this task `scope` is validated and echoed, `scope="bogus"` errors instead of being swallowed, and the param probe reaches `fix`/`offset`. **No finding moves yet** — that is Task 2 onward. Splitting here means a reviewer can reject the plumbing without rejecting the semantics.

**Files:**
- Modify: `src/librarian/tools/doctor.rs:326-350` (`call`'s argument reads)
- Modify: `src/librarian/tools/doctor.rs:751-766` (`limit`/`offset` reads)
- Modify: `src/librarian/tools/librarian.rs:64-68` (schema prose), `:234-239` (probe exemption)
- Test: `src/librarian/tools/doctor.rs` inline `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `super::scope::{resolve_scope, Scope, UmbrellaPolicy}` — `resolve_scope(requested: Option<Scope>, current: Option<&CurrentProject>, policy: UmbrellaPolicy, default: Scope) -> Result<(Scope, bool)>`.
- Produces: `struct Args` with fields `scope: Option<Scope>`, `fix: Option<String>`, `confirm: bool`, `root: Option<String>`, `old_root: Option<String>`, `new_root: Option<String>`, `limit: Option<usize>`, `offset: Option<usize>`. Task 2 consumes `Args::scope`. The response gains a top-level `"scope"` object shaped by `ScopeApplied::to_json()` plus a sibling `"scope_fallback": bool`.

- [ ] **Step 1: Write the failing tests**

Add to `doctor.rs`'s `mod tests`. `TestToolContextBuilder` and `unscoped_ctx()` already exist there (`unscoped_ctx` at `:12374`).

```rust
/// `scope` was declared in the shared schema with `"default": "project"` and read by
/// nothing (`d4b61746950b86b7`). These three pin the plumbing: a bad value must be
/// refused rather than swallowed, and the applied scope must be readable in the
/// response — without which a caller cannot tell a scoped result from an unscoped one.
#[tokio::test]
async fn an_unknown_scope_value_is_refused_rather_than_ignored() {
    let (_tmp, root, _live) = git_fixture_with_commit();
    let ctx = ctx_at(&root);
    let err = call(&ctx, json!({ "scope": "galaxy" }))
        .await
        .expect_err("an unknown scope must be a RecoverableError, not a silent default");
    let msg = err.to_string();
    assert!(
        msg.contains("scope"),
        "the refusal must name the offending param; got: {msg}"
    );
}

#[tokio::test]
async fn the_applied_scope_is_echoed_in_the_response() {
    let (_tmp, root, _live) = git_fixture_with_commit();
    let ctx = ctx_at(&root);

    let absent = call(&ctx, json!({})).await.unwrap();
    assert_eq!(
        absent["scope"]["applied"], "project",
        "omitted scope must resolve to project — scope.rs:11 states that default for \
         every listing surface"
    );
    assert_eq!(absent["scope_fallback"], json!(false));

    let explicit = call(&ctx, json!({ "scope": "repo" })).await.unwrap();
    assert_eq!(explicit["scope"]["applied"], "repo");
}

/// The no-active-project path. `resolve_scope` widens `project`/`repo` to `All` and
/// sets the fallback flag; a caller must be able to see that their narrow request came
/// back broad, or they will read a machine-wide report as their own project's.
#[tokio::test]
async fn a_project_request_without_an_active_project_reports_its_fallback() {
    let ctx = unscoped_ctx();
    let v = call(&ctx, json!({ "scope": "project" })).await.unwrap();
    assert_eq!(v["scope"]["applied"], "all");
    assert_eq!(
        v["scope_fallback"], json!(true),
        "a silently widened scope is the defect this flag exists to prevent"
    );
}
```

`ctx_at(&root)` is a helper this task must add beside `unscoped_ctx()` — the existing tests build contexts inline:

```rust
/// A ToolContext whose `current_project` is `root`, for scope tests. Mirrors what
/// `LibrarianAdapter::derive_ctx` produces for a plain (non-worktree) checkout.
fn ctx_at(root: &std::path::Path) -> ToolContext {
    TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_current_project(crate::librarian::current_project::CurrentProject {
            abs_path: root.to_path_buf(),
            git_root: root.to_path_buf(),
            main_root: None,
            umbrella: None,
        })
        .build()
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```
cargo test --workspace doctor::tests::an_unknown_scope_value_is_refused_rather_than_ignored \
                        doctor::tests::the_applied_scope_is_echoed_in_the_response \
                        doctor::tests::a_project_request_without_an_active_project_reports_its_fallback
```

Expected: all three FAIL. The first two on a compile error if `TestToolContextBuilder` has no `with_current_project` — **check that builder's actual methods before writing `ctx_at`** and adapt; the surrounding tests at `:11919` and `:14535` already build project-bearing contexts, so copy whichever form they use rather than inventing one. Once compiling: test 1 fails because `call` returns `Ok`, tests 2 and 3 because `v["scope"]` is `Null`.

- [ ] **Step 3: Add the typed `Args` and resolve the scope**

Replace the untyped reads at the head of `call`. Keep `run_fix`'s dispatch behaviour identical.

```rust
/// Every argument `doctor` accepts, typed.
///
/// Typed rather than read through `args.get(...)`: the untyped form is what let a
/// declared `scope` be discarded in silence (`d4b61746950b86b7`), and it is what
/// exempted `fix`/`offset` from the `librarian.rs` param probe — an exemption whose
/// own comment predicted this bug in writing.
///
/// `#[serde(default)]` on every field, and NO `deny_unknown_fields`: the librarian
/// tool has one flat schema shared by 11 actions, so `doctor` legitimately receives
/// sibling actions' params. Denying unknown fields here would refuse
/// schema-conformant calls.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Args {
    scope: Option<super::scope::Scope>,
    fix: Option<String>,
    confirm: bool,
    root: Option<String>,
    old_root: Option<String>,
    new_root: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let args: Args = serde_json::from_value(args).map_err(|e| {
        RecoverableError::with_hint(
            format!("doctor: bad args: {e}"),
            "scope must be one of project|repo|umbrella|all; limit/offset are integers; \
             see librarian(action=\"doctor\") in the input schema",
        )
    })?;

    if let Some(fix) = args.fix.as_deref() {
        // `old_root` is the self-documenting name; `root` stays a back-compat fallback.
        let old_root_arg = args.old_root.as_deref().or(args.root.as_deref());
        return run_fix(ctx, fix, old_root_arg, args.new_root.as_deref(), args.confirm).await;
    }

    let (effective_scope, scope_fallback) = super::scope::resolve_scope(
        args.scope,
        ctx.current_project.as_deref(),
        // Require, not Literal: `doctor` is a search-shaped surface like `find`, so
        // `all` without an umbrella is a request to widen with nothing to widen to.
        // `context` uses Literal because reaching across every project is its point.
        super::scope::UmbrellaPolicy::Require,
        super::scope::Scope::Project,
    )?;
    // ... existing body, unchanged for now ...
```

Then at the two later read sites, replace `args.get("limit")...` with `args.limit` and `args.get("offset")...` with `args.offset`:

```rust
    let sample_limit = args.limit.unwrap_or(OUTSIDE_ROOTS_SAMPLE_DEFAULT);
    let sample_offset = args.offset.unwrap_or(0);
```

And add the echo where the report object is assembled, beside `summary` and `catalog_health`:

```rust
    let applied = super::scope::ScopeApplied {
        scope: effective_scope,
        abs_path: ctx.current_project.as_deref().map(|c| c.abs_path.clone()),
        git_root: ctx.current_project.as_deref().map(|c| c.git_root.clone()),
        umbrella: ctx.current_project.as_deref().and_then(|c| c.umbrella.clone()),
    };
    // ... "scope": applied.to_json(), "scope_fallback": scope_fallback, ...
```

- [ ] **Step 4: Fix the probe's label selector, THEN delete the exemption**

**Read this before editing either file.** Appending `/doctor` to `scope`'s description is
**inert**, and looks like the fix. `param_probe::sweep` resolves a key's action with
`desc.split(':').next().and_then(|l| l.split('/').next())` (`src/tools/param_probe.rs:108`)
— **the first slash token only, with no loop over the rest.** `scope`'s label begins
`context/…`, so it is probed as `context:scope` and for no other action. Adding `doctor`
at the end of that list changes nothing the probe reads.

Verified 2026-09-09, and the measurement is the point: the probe test
`every_action_labelled_schema_key_is_honored_by_that_action` is **green** while `doctor`
demonstrably discards `scope`. Had the probe reached `doctor:scope` it would have found
`base == probed` and reported it unhonored, so green *proves* the pair is unchecked.

So this step is two edits, in this order:

1. **`src/tools/param_probe.rs`** — iterate every slash token instead of taking the first:

```rust
        // Every action a shared key names, not just the first. `desc.split('/').next()`
        // checked one action per key and nothing recorded that the rest were unchecked —
        // so `scope`, labelled for four actions, was probed for `context` alone. An
        // unchecked pair that is not in `accepts_any_json` is worse than an admitted one:
        // the admission list is where blindness is declared, and this blindness was
        // undeclared. See
        // docs/issues/archive/2026-09-09-param-probe-checks-one-action-per-shared-key.md
        let Some(label) = desc.split(':').next() else { continue };
        for action in label.split('/') {
            if !spec.actions.contains(&action) {
                continue;
            }
            // ... the existing base/probe pair, per action ...
        }
```

`checked` must increment **per action-key pair**, not per key, so the `floor` becomes a
count of pairs. Raise the `floor` at all six call sites to the new counts rather than
lowering it — a floor that still passes after the selector widens tells you nothing.

2. **`src/librarian/tools/librarian.rs`** — now that the label is read in full, add
`doctor` to it and drop the exemption. At `:234-239` remove the comment block and change
`accepts_any_json: &["fix", "offset"],` to `accepts_any_json: &[],`. At `:64-68` replace
the `scope` description with:

```
"description": "context/reindex/workspace_state_at/link_scan/doctor: scope. audit_doc_refs: project-scoped only in v1 — any other value is rejected. Defaults to the active project on every action; `reindex` alone widens to `all` when no project is active, since there is then no project to re-scan. On `doctor` the reported worklist narrows while catalog_health's cross-repo metrics stay global."
```

**Expect this step to red other tools.** Widening the selector newly probes
`reindex:scope`, `workspace_state_at:scope`, `link_scan:scope`,
`workspace_state_at:include_archived` and every second-and-later action of any
`.`-separated multi-action description (`limit` names four actions and is probed for
`legibility_scan` alone today). Each red is a real IC-15 instance, not collateral —
**do not narrow the selector to get green, and do not move a red key into
`accepts_any_json` unless you have read its handler and confirmed it reads the param
through an untyped accessor.** If the reds are numerous, stop and report them rather than
fixing them inside this task; they are other actions' bugs and want their own files.

- [ ] **Step 5: Run the tests to verify they pass**

```
cargo test --workspace doctor::tests::an_unknown_scope
cargo test --workspace doctor::tests::the_applied_scope_is_echoed
cargo test --workspace doctor::tests::a_project_request_without_an_active_project
cargo test --workspace librarian::tools::librarian
```

Expected: all PASS. The last one is the param probe, which now reaches `fix` and `offset` — **if it reds, that is a real finding, not collateral.** It means one of those params accepts an ill-typed value. `fix: Option<String>` rejects `fix=[]`, which the deleted comment named as a known hole (*"`doctor(fix=[])` runs a read-only scan and reports success rather than refusing"*), so the probe should now be satisfied. Do not re-add the exemption to get green; fix the type.

- [ ] **Step 6: Run the full gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
# read BOTH exit codes; confirm your own test names appear in the DEFAULT lane:
#   grep -E '^test librarian::tools::doctor::tests::.*scope' <default-lane-output>
git add src/librarian/tools/doctor.rs src/librarian/tools/librarian.rs
git commit -m "$(cat <<'EOF'
feat(doctor): type doctor's args so a declared scope cannot be discarded

`scope` was declared in the shared librarian schema with `"default": "project"`
and read by nothing — `scope="all"` returned a project-scoped report with no
error, under-reporting by 636 rows. Root cause was the untyped `args.get(...)`
reads, which also exempted `fix`/`offset` from the param probe; that exemption's
own comment predicted this bug.

No finding moves yet: this accepts, validates and echoes the scope. The
worklist narrowing follows.

Bug: d4b61746950b86b7
Refs: bug-fix-session-log:F-125, W-117

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: `DoctorScope` — one scoping unit, applied at the SQL layer

> **⚠ SEQUENCING CORRECTION 2026-09-10 — DO TASK 4 FIRST. Task 2 cannot land before it.**
> Attempted as written and reverted; three findings, each verified at the bytes rather
> than reasoned about.
>
> 1. **Steps 4 and 5 contradict each other.** Step 4 splices `scope.sql_and()` into
>    `scan_artifact_paths`'s query. That scan produces `outside_roots_by_project`, whose
>    contract — Ruling 17, restated in the function's own doc comment at
>    `src/librarian/tools/doctor.rs` — is that the METRIC stays global while the WORKLIST
>    narrows. A `WHERE` clause removes the foreign rows before they can be counted, so
>    the metric empties at `scope=project`, and **Step 5's own assertion
>    (`the cross-repo metric must stay global at scope=project`) fails against Step 4.**
>
> 2. **Narrowing per-row instead breaks a different announcement.** Gating the row checks
>    on `scope.admit(...)` preserves the metric but reds
>    `row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not` with *"the
>    drop must be announced, not silent"* — pre-filtering removes violations before the
>    `SCOPED_ROW_CHECKS` `retain` can count and announce them. **That retain is what Task
>    4 retires.** Hence the swap: unify the announcements first, then Task 2's wiring is a
>    one-line splice.
>
>    The general property, which is why all three attempts failed the same way: `doctor`
>    has five scoping mechanisms and **each owns its own announcement**, so any sixth
>    narrowing applied before they are unified takes an announcement with it. Not three
>    bugs — one property, met three times.
>
> 3. **Task 2 is NOT "shippable alone", and the toolchain says so.** `cargo clippy
>    --workspace --all-targets -- -D warnings` refuses to compile a `DoctorScope` no
>    caller uses: *"associated function `new` is never used"*, `-D dead-code`. So Steps
>    1–3 cannot be committed as a unit; the struct and its first consumer must land
>    together. Do not answer this with `#[allow(dead_code)]` — that suppresses the guard
>    that is correctly reporting the sequencing problem.
>
> **Fixture corrections for whoever picks this up** (Step 1 as written does not compile):
> `insert_artifact_row` and `ctx_at` do not exist. The real helpers are `seed_artifact`
> and `ctx_rooted_at`, and both are private to `doctor.rs`'s own `#[cfg(test)] mod tests`,
> so a sibling `doctor/scope.rs` cannot reach them — write local twins. Also, `doctor`
> resolves scope with `UmbrellaPolicy::Require`, so Step 5's `scope="all"` call is
> **refused outright** without a configured umbrella (*"scope=\"all\" requires a
> configured umbrella"*); the fixture needs `cp.umbrella = Some(..)` plus
> `.with_umbrellas(vec![Umbrella { name, members }])`.
>
> No rename is needed: edition 2021 lets `doctor.rs` and `doctor/scope.rs` coexist, so
> `mod scope;` in `doctor.rs` is the whole wiring — the 15,620-line file stays put.
>
> Reverted rather than landed behind `#[ignore]` + `#[allow(dead_code)]`: both would be
> green-looking states that suppress the two guards actually reporting the problem.
> — sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`

**Files:**
- Create: `src/librarian/tools/doctor/scope.rs`
- Modify: `src/librarian/tools/doctor.rs:1714-1772` (`scan_artifact_paths`), `:326` (`call` builds and threads it)
- Test: `src/librarian/tools/doctor/scope.rs` inline `mod tests`, plus one integration test in `doctor.rs`'s `mod tests`

**Interfaces:**
- Consumes: `Args::scope` and the `effective_scope` from Task 1; `super::super::scope::apply_scope`; `crate::librarian::filter::compile`; `super::super::containing_root`.
- Produces:

```rust
pub(super) struct DoctorScope {
    pub scope: Scope,
    roots: Vec<PathBuf>,               // empty ⇒ Scope::All ⇒ everything in scope
    sql: Option<SqlFragment>,          // None ⇒ no WHERE narrowing
    scoped_out: BTreeMap<String, usize>,
}

impl DoctorScope {
    pub(super) fn new(scope: Scope, ctx: &ToolContext) -> Result<Self>;
    /// Lexical predicate for a path already in hand.
    pub(super) fn contains(&self, abs_path: &Path) -> bool;
    /// `contains`, and on false tallies the row under its project group.
    pub(super) fn admit(&mut self, abs_path: &str) -> bool;
    /// ` AND (<frag>)` to splice into a raw query, plus its bound params.
    pub(super) fn sql_and(&self) -> (String, Vec<rusqlite::types::Value>);
    pub(super) fn scoped_out(&self) -> &BTreeMap<String, usize>;
}
```

Tasks 3–6 consume `admit` and `sql_and`. Task 8 consumes `contains`.

- [ ] **Step 1: Write the failing tests**

Create `src/librarian/tools/doctor/scope.rs` with the module doc and tests only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// `Scope::All` must narrow NOTHING — the property that keeps `scope="all"` a real
    /// widening rather than a differently-shaped default.
    #[test]
    fn scope_all_admits_every_path_and_binds_no_sql() {
        let ctx = unscoped_ctx();
        let mut s = DoctorScope::new(Scope::All, &ctx).unwrap();
        assert!(s.admit("/anywhere/at/all/docs/x.md"));
        let (sql, params) = s.sql_and();
        assert_eq!(sql, "", "All must contribute no WHERE clause");
        assert!(params.is_empty());
        assert!(s.scoped_out().is_empty());
    }

    /// The discriminating property, at unit grain: a foreign path is refused AND
    /// counted. Counting is not incidental — Ruling 17 requires the metric to survive
    /// the narrowing, and a dropped row that is not tallied is invisible.
    #[test]
    fn scope_project_refuses_a_foreign_path_and_tallies_it() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mine");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        let ctx = ctx_at(&root);

        let mut s = DoctorScope::new(Scope::Project, &ctx).unwrap();
        assert!(s.admit(&root.join("docs/mine.md").to_string_lossy()));
        assert!(!s.admit("/home/other/repo/docs/theirs.md"));
        assert_eq!(s.scoped_out().values().sum::<usize>(), 1);
        assert!(
            s.contains(&root.join("docs/mine.md")),
            "contains must agree with admit"
        );
    }

    /// The component-boundary guarantee `containing_root` provides, restated here
    /// because `DoctorScope` is the layer callers now use. `/proj/sub` must not be
    /// treated as contained by `/proj/subterfuge`.
    #[test]
    fn a_sibling_directory_with_a_shared_prefix_is_not_in_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let ctx = ctx_at(&root);
        let mut s = DoctorScope::new(Scope::Project, &ctx).unwrap();
        let sibling = format!("{}terfuge/docs/x.md", root.to_string_lossy());
        assert!(!s.admit(&sibling));
    }

    /// The SQL half must bind the same decision as the lexical half, or a scan that
    /// queries and a scan that filters in Rust will disagree about the same row.
    #[test]
    fn the_sql_fragment_selects_exactly_what_contains_admits() {
        let cat = Catalog::open_in_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mine");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        let mine = root.join("docs/mine.md");
        let theirs = std::path::PathBuf::from("/home/other/repo/docs/theirs.md");
        insert_artifact_row(&cat.conn, &mine);
        insert_artifact_row(&cat.conn, &theirs);

        let ctx = ctx_at(&root);
        let s = DoctorScope::new(Scope::Project, &ctx).unwrap();
        let (and_clause, params) = s.sql_and();
        let sql = format!("SELECT abs_path FROM artifact WHERE 1=1{and_clause}");
        let mut stmt = cat.conn.prepare(&sql).unwrap();
        let got: Vec<String> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();

        assert_eq!(got.len(), 1, "only the in-scope row may be selected; got {got:?}");
        assert!(s.contains(&mine) && !s.contains(&theirs));
    }
}
```

`insert_artifact_row` is a helper `doctor.rs`'s tests already use for path fixtures — locate it (`grep -n 'fn insert_artifact_row' src/librarian/tools/doctor.rs`) and import it, or copy its two-line `INSERT` if it is not `pub(super)`.

- [ ] **Step 2: Run the tests to verify they fail**

```
cargo test --workspace doctor::scope::tests
```

Expected: FAIL to compile — `DoctorScope` does not exist. Then, once Step 3 lands, each test must be seen to fail *before* its branch is written if you implement incrementally.

- [ ] **Step 3: Implement `DoctorScope`**

```rust
//! One scoping unit for every `doctor` check.
//!
//! Before this module, Ruling 17 — *the metric stays global, the worklist is the
//! active developer's* — was implemented five times in five shapes: an in-loop
//! known-roots filter, four per-scan `ctx` filters each with its own tally map, a
//! fifth of that shape for `cited_prefix_with_no_definer`, a post-hoc `retain` over
//! a seven-string list of check names, and six inline `containing_root` calls that
//! narrowed correctly and **published no count at all**. Two of them keyed off
//! `cp.git_root` while `scope::Scope::Project` keys off `cp.abs_path`, so "scoped to
//! the project" meant two different things inside one tool.
//!
//! It carries BOTH halves on purpose. Scans that issue their own query splice
//! [`DoctorScope::sql_and`] and never read a foreign row; scans that already hold a
//! path from a shared row loop call [`DoctorScope::admit`]. One decision, two
//! spellings of it, asserted equal by
//! `the_sql_fragment_selects_exactly_what_contains_admits` — because a lexical
//! predicate and a `LIKE` clause disagreeing about one row would be invisible.
//!
//! The SQL half is what removes the cost, not just the noise:
//! `scan_terminal_status_without_fix_anchor` used to `read_to_string` every terminal
//! bug file in a catalog spanning 112 project roots and *then* drop the foreign ones.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::super::scope::{apply_scope, Scope};
use super::super::{containing_root, ToolContext};
use crate::librarian::filter::{compile, SqlFragment};

pub(super) struct DoctorScope {
    pub scope: Scope,
    roots: Vec<PathBuf>,
    sql: Option<SqlFragment>,
    scoped_out: BTreeMap<String, usize>,
}

impl DoctorScope {
    pub(super) fn new(scope: Scope, ctx: &ToolContext) -> Result<Self> {
        // `apply_scope` owns the umbrella lookup and the worktree-overlay OR clause;
        // re-deriving either here is how the two definitions of "project" diverged.
        let (filter, applied) = apply_scope(None, scope, &ctx.workspace, ctx.current_project.as_deref(), &[])?;
        let sql = match &filter {
            Some(f) => Some(compile(f)?),
            None => None,
        };
        // The lexical twin of that clause. `Scope::Project` over a linked worktree
        // spans BOTH the worktree and its main checkout, matching `apply_scope`'s
        // deliberate over-selection.
        let mut roots = Vec::new();
        if !matches!(scope, Scope::All) {
            if let Some(cp) = ctx.current_project.as_deref() {
                let primary = match scope {
                    Scope::Repo => &cp.git_root,
                    _ => &cp.abs_path,
                };
                roots.push(primary.clone());
                if let Some(main) = &cp.main_root {
                    roots.push(main.clone());
                }
                if matches!(scope, Scope::Umbrella) {
                    roots.extend(umbrella_member_roots(ctx));
                }
            }
        }
        let _ = applied;
        Ok(Self { scope, roots, sql, scoped_out: BTreeMap::new() })
    }

    pub(super) fn contains(&self, abs_path: &Path) -> bool {
        if self.roots.is_empty() {
            return true;
        }
        containing_root(&self.roots, abs_path).is_some()
    }

    pub(super) fn admit(&mut self, abs_path: &str) -> bool {
        if self.contains(Path::new(abs_path)) {
            return true;
        }
        *self.scoped_out.entry(super::outside_roots_group(abs_path)).or_insert(0) += 1;
        false
    }

    pub(super) fn sql_and(&self) -> (String, Vec<rusqlite::types::Value>) {
        match &self.sql {
            Some(f) => (format!(" AND ({})", f.sql), f.params.clone()),
            None => (String::new(), Vec::new()),
        }
    }

    pub(super) fn scoped_out(&self) -> &BTreeMap<String, usize> {
        &self.scoped_out
    }
}
```

`umbrella_member_roots(ctx)` must be written to read `ctx.workspace.umbrellas`, matching how `apply_scope`'s `Scope::Umbrella` arm resolves members (`src/librarian/tools/scope.rs:180-200`) — **read that arm and mirror it exactly**; do not approximate it, or the lexical and SQL halves will disagree and `the_sql_fragment_selects_exactly_what_contains_admits` will red for `Umbrella`. Add `mod scope;` to `doctor.rs` and convert `doctor.rs` to `doctor/mod.rs` if it is not already a directory module.

- [ ] **Step 4: Convert `scan_artifact_paths` to it**

This scan is first because the existing `known_elsewhere` filter already sits inside its row loop, so the shape is closest. Change the signature and the outside-roots branch:

```rust
fn scan_artifact_paths(
    conn: &rusqlite::Connection,
    roots: &[PathBuf],
    known_elsewhere: &[PathBuf],
    scope: &mut DoctorScope,
) -> Result<(Vec<Violation>, BTreeMap<String, usize>)> {
    let (and_clause, params) = scope.sql_and();
    let sql = format!("SELECT id, abs_path FROM artifact WHERE 1=1{and_clause} ORDER BY abs_path");
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<(String, String)> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    // ... row loop unchanged ...
```

`ORDER BY abs_path` is load-bearing — the outside-roots sample is paged by `offset` and an unordered window reshuffles between calls. Keep it.

**Keep the `known_elsewhere` branch.** It answers a different question from `DoctorScope`: whether a row belongs to a workspace this machine knows about *at all*. At `Scope::All` the SQL narrows nothing and that branch is still the discriminator between "another workspace's row" and "orphan".

- [ ] **Step 5: Write and run the integration test**

```rust
/// The end-to-end discriminator, and the assertion this whole change exists to make
/// true. NOTE the strict inequality: an `assert_eq!(all, project)` here passes
/// against the bug and is monotone under it — which is exactly how the defect
/// survived (`bug-fix-session-log:F-125`).
#[tokio::test]
async fn scope_all_reports_strictly_more_than_scope_project() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mine");
    std::fs::create_dir_all(root.join("docs")).unwrap();
    let ctx = ctx_at(&root);
    // A row belonging to no known root, and no managed root — so it fires
    // `abs_path_outside_managed_roots` at `all` and is scoped out at `project`.
    insert_artifact_row(&ctx.catalog.lock().conn, Path::new("/home/other/repo/docs/x.md"));

    let project = call(&ctx, json!({ "scope": "project" })).await.unwrap();
    let all = call(&ctx, json!({ "scope": "all" })).await.unwrap();

    let p = project["summary"]["total"].as_u64().unwrap();
    let a = all["summary"]["total"].as_u64().unwrap();
    assert!(a > p, "scope=all must report strictly more than scope=project; got all={a} project={p}");

    // Ruling 17: the metric must NOT have shrunk with the worklist.
    assert!(
        !project["catalog_health"]["outside_roots_by_project"]
            .as_object()
            .unwrap()
            .is_empty(),
        "the cross-repo metric must stay global at scope=project"
    );
}
```

```
cargo test --workspace doctor::scope::tests
cargo test --workspace doctor::tests::scope_all_reports_strictly_more_than_scope_project
```
Expected: PASS.

- [ ] **Step 6: Run the full gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/doctor/ src/librarian/tools/doctor.rs
git commit -m "$(cat <<'EOF'
feat(doctor): add DoctorScope and narrow scan_artifact_paths at the SQL layer

One scoping unit carrying both halves of the same decision: a compiled
SqlFragment for scans that query, a lexical predicate for scans that hold a
path, asserted equal so the two cannot disagree about a row. Replaces the
first of five bespoke mechanisms.

The SQL half removes the cost as well as the noise — scans no longer read
foreign files before dropping them.

Ruling 17 preserved: outside_roots_by_project still counts the scoped-out rows.

Bug: d4b61746950b86b7
Refs: bug-fix-session-log:W-117

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Convert the entry-validity family and `cited_prefix_with_no_definer`

Five scans, each currently carrying its own `ctx` filter and tally map. Each conversion is a concrete substitution at a named site, not a repetition of Task 2.

**Files:**
- Modify: `src/librarian/tools/doctor.rs:3164` (`scan_conditional_past_due`), `:3314` (`scan_dated_stale`), `:3439` (`scan_cited_but_undeclared`), `:3544` (`scan_validity_unparseable`), `:3969` (`scan_cited_prefix_with_no_definer`), and `call`'s fold at `:470-478`
- Test: `src/librarian/tools/doctor.rs` `mod tests`

**Interfaces:**
- Consumes: `DoctorScope::admit`, `DoctorScope::scoped_out` from Task 2.
- Produces: each of the five scans loses its `(Vec<Violation>, BTreeMap<String, usize>)` return in favour of `Result<Vec<Violation>>`, taking `scope: &mut DoctorScope` instead of `ctx: &ToolContext`. `call` reads one tally from `scope.scoped_out()` rather than folding five.

- [ ] **Step 1: Write the failing test**

```rust
/// `entry_indegree` must stay corpus-wide while the four validity checks narrow.
/// Narrowing the METRIC would make a foreign citer invisible and manufacture a false
/// negative — the specific error Ruling 17 names.
#[tokio::test]
async fn the_validity_family_narrows_its_worklist_but_not_its_exposure_metric() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mine");
    std::fs::create_dir_all(root.join("docs/trackers")).unwrap();
    let ctx = ctx_at(&root);
    // A local ledger with a past-due conditional entry, cited from a FOREIGN artifact.
    // The finding is local (reported at project scope); its exposure count is not.
    seed_past_due_conditional(&ctx, &root, /* foreign_citers = */ 2);

    let v = call(&ctx, json!({ "scope": "project" })).await.unwrap();
    assert!(
        v["summary"]["by_check"]["entry_conditional_past_due"].as_u64().unwrap() >= 1,
        "a LOCAL past-due entry must still be reported at project scope"
    );
    let detail = find_violation(&v, "entry_conditional_past_due").unwrap();
    assert!(
        detail["detail"].as_str().unwrap().contains("2"),
        "the exposure count must remain cross-repo; got {detail:?}"
    );
}
```

`seed_past_due_conditional` and `find_violation` are fixtures this task must add; model the ledger body on the existing fixture in `call_reports_entry_validity_scoped_out_rows_in_catalog_health` (`:14535`), which already builds a validity-bearing tracker — **read it and reuse its body text**, because the `**Valid:** conditional — <event>` grammar is strict (`dated` takes no trailing text; only `conditional` carries an em-dash tail) and an invalid declaration lands in `validity_unparseable` instead of the check under test.

- [ ] **Step 2: Run to verify it fails**

```
cargo test --workspace doctor::tests::the_validity_family_narrows_its_worklist_but_not_its_exposure_metric
```
Expected: FAIL (fixture helpers undefined, then the detail assertion once they compile).

- [ ] **Step 3: Convert each of the five scans**

For each site, the substitution is identical in shape. `scan_conditional_past_due` (`:3164`) shows it:

```rust
// before
fn scan_conditional_past_due(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
    indegree: &BTreeMap<String, usize>,
) -> Result<(Vec<Violation>, BTreeMap<String, usize>)> {
    let Some(cp) = ctx.current_project.as_deref() else { return Ok((Vec::new(), Default::default())) };
    // ... per-row: if containing_root(&[cp.git_root], path).is_none() { tally; continue }

// after
fn scan_conditional_past_due(
    scope: &mut DoctorScope,
    conn: &rusqlite::Connection,
    indegree: &BTreeMap<String, usize>,
) -> Result<Vec<Violation>> {
    // ... per-row: if !scope.admit(abs_path) { continue }
```

`indegree` stays exactly as it is — built by `entry_indegree(&cat.conn)` with **no** scope argument. Do not thread `scope` into it.

Apply the same substitution at `:3314`, `:3439`, `:3544`, `:3969`.

- [ ] **Step 4: Collapse `call`'s five-way fold**

Delete the `entry_validity_scoped_by_project` and `cited_prefix_scoped_by_project` accumulation blocks (`:470-478` and the `cited_prefix_scoped` handling) and read the single tally:

```rust
    // One tally, from one mechanism. The per-check maps existed because each scan
    // owned its own filter; with one filter there is one map.
    let scoped_by_project = scope.scoped_out().clone();
```

**Keep both `catalog_health` keys in the response**, populated from that one map. They are documented in `docs/conventions/` and read by the hint text; collapsing the *mechanism* must not silently rename the *output*.

- [ ] **Step 5: Run to verify it passes**

```
cargo test --workspace doctor::tests
```
Expected: PASS, including the pre-existing `call_reports_entry_validity_scoped_out_rows_in_catalog_health` (`:14535`) and `call_accumulates_scoped_out_counts_per_root_across_checks_and_keeps_roots_distinct` (`:14584`). **If the latter reds, read it before changing it** — it pins the fold this task collapses, so it may need its construction updated while keeping its assertion. Do not delete it.

- [ ] **Step 6: Gate and commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/doctor/ && git commit -m "$(cat <<'EOF'
refactor(doctor): route the validity family and cited_prefix through DoctorScope

Five scans lose their bespoke ctx filters and tally maps for one shared
mechanism. entry_indegree stays corpus-wide — narrowing the exposure metric
would hide a foreign citer and manufacture the false negative Ruling 17 names.

Bug: d4b61746950b86b7

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Retire `SCOPED_ROW_CHECKS` and admit the two checks that started firing

> **⚠ PROMOTED TO FIRST 2026-09-10 — this task now precedes Task 2.** Derivation in Task
> 2's banner. Short form: `doctor` has five scoping mechanisms and each owns its own
> announcement, so a sixth narrowing added before they are unified silently takes an
> announcement with it. Measured twice — an SQL splice empties
> `outside_roots_by_project`, a per-row gate reds
> `row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not` with *"the drop
> must be announced, not silent"*. Both are Ruling 17 failures introduced by the fix for
> Ruling 17.
>
> Land `DoctorScope` (Task 2 Steps 1–3) **in this task's commit**, not before it: clippy's
> `-D dead-code` refuses a struct with no consumer, so the unit and its first caller are
> one commit whether or not the plan splits them.

> **⚠ SUPERSEDED 2026-09-10 by `b057cc6d` — Steps 1–5 below are wrong and were not
> followed.** The measured half of this task (scope `params_behind_body` and
> `params_status_drift`) shipped; the mechanism change did not, because two verifications
> at the bytes falsified it.
>
> 1. **Membership must be keyed by CHECK NAME, not by scan.** `frontmatter_id_mismatch`
>    *and* `frontmatter_id_is_not_a_catalog_id` are emitted from inside
>    `scan_artifact_paths`' row loop (`:1796`; the function spans `:1756-1968`) — a scan
>    that must **not** be scoped wholesale, since it owns `outside_roots_by_project`. A
>    per-scan `admit` gate cannot express *"these two names from that scan, and nothing
>    else it emits"*, which is precisely what `SCOPED_ROW_CHECKS` does. The const is
>    load-bearing, not legacy.
> 2. **Step 3 names the wrong function.** `scan_frontmatter_id_mismatches` has exactly one
>    non-test caller — `run_fix` at `:1354` — so it is never reached in the report path.
>    Gating it narrows a repair that is already root-scoped and changes the report by
>    nothing, while both check names above lose their scoping entirely. The existing guard
>    `row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not` seeds
>    `frontmatter_id_mismatch`, so it catches one of the two; `frontmatter_id_is_not_a_catalog_id`
>    has no scoping test at all and the regression would have shipped silently.
> 3. **Blast radius unpriced.** The nine scans have ~65 call sites, ~50 of them direct unit
>    tests on `(&cat.conn)`. "Add `scope: &mut DoctorScope`" is nine signatures and ~56
>    edits, not nine.
>
> Corrections to the numbers and fixtures, measured 2026-09-10 at HEAD `903e2332`:
> `params_behind_body` is **3** findings (not 4), still 2 foreign — the foreign counts in
> the prose below are right and the total moved. `seed_params_behind_body` and `ctx_at` do
> not exist; use `seed_tracker(&cat, id, dir, body, ids)` and `ctx_rooted_at(cat, &root)`.
> Seeding two byte-identical trackers under different roots fires **two** scoped checks per
> ledger, not one, so Step 1's `assert_eq!(…, 1)` on the tally is wrong — assert the kept
> and dropped counts are equal instead, with a `>= 1` floor against `0 == 0`.
>
> A defect found while doing it and fixed in the same commit: the hint sentence naming the
> scoped-out checks listed **six of the seven**, omitting `frontmatter_status_mismatch`. It
> is now generated from the const, which is typed `&[Check]`.

**Files:**
- Modify: `src/librarian/tools/doctor.rs:551-587` (delete the const and the `retain`), `:4308` (`scan_params_behind_body`), `:4518` (`scan_params_status_drift`), and the seven scans the const named
- Test: `src/librarian/tools/doctor.rs` `mod tests`

**Interfaces:**
- Consumes: `DoctorScope::admit`.
- Produces: no new API. The `retain` and the `const SCOPED_ROW_CHECKS: &[&str]` are gone; `row_checks_scoped_by_project` is populated from the shared tally.

The two additions are **prescribed by measurement, not guessed.** `docs/issues/archive/2026-08-27-doctor-still-reports-52pct-foreign-rows-via-six-other-checks.md` § *Known gap, deliberately not swept* left `params_behind_body`, `params_status_drift`, `snapshot_drift` and `augmentation_declared_but_absent` out because they reported **zero** findings, and wrote: *"If one of them starts firing across repos, add it to `SCOPED_ROW_CHECKS` — and read its repair path first."* Measured 2026-09-09: `params_behind_body` = 4 findings, **2 foreign** (`work/mirela/backend-kotlin`, `work/stefanini/southpole`); `params_status_drift` = 3, **1 foreign** (`work/mirela/eduplanner-ui`). `snapshot_drift` and `augmentation_declared_but_absent` are still **0** — convert them anyway, since with one mechanism there is no per-check opt-in list to be absent from, but do **not** claim they were measured firing.

The repair-path read that instruction demands: neither `params_behind_body` nor `params_status_drift` has a `fix=` mode. Their remedies are caller-side `doc(action="augment", merge=true)` and `doc(action="update_entry")` calls named in each violation's `detail`. So scoping their report scopes nothing else — unlike `worktree_scoped_row`, which is Task 7.

- [ ] **Step 1: Write the failing test**

```rust
/// The measured foreign leak this task closes. `params_behind_body` fires on rows in
/// three other repos; at project scope none may appear, and at `all` they must.
#[tokio::test]
async fn params_behind_body_is_scoped_like_every_other_row_check() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mine");
    std::fs::create_dir_all(root.join("docs/trackers")).unwrap();
    let ctx = ctx_at(&root);
    // A FOREIGN ledger whose body claims an id its params lack.
    seed_params_behind_body(&ctx, Path::new("/home/other/repo/docs/trackers/t.md"));

    let project = call(&ctx, json!({ "scope": "project" })).await.unwrap();
    assert_eq!(
        project["summary"]["by_check"]["params_behind_body"], json!(0),
        "another repo's params drift is not this developer's worklist"
    );
    assert_eq!(
        project["catalog_health"]["row_checks_scoped_by_project"]
            .as_object().unwrap().values()
            .filter_map(|v| v.as_u64()).sum::<u64>(),
        1,
        "the drop must be counted, not silent"
    );

    let all = call(&ctx, json!({ "scope": "all" })).await.unwrap();
    assert_eq!(all["summary"]["by_check"]["params_behind_body"], json!(1));
}
```

- [ ] **Step 2: Run to verify it fails**

```
cargo test --workspace doctor::tests::params_behind_body_is_scoped_like_every_other_row_check
```
Expected: FAIL — at project scope the count is 1, not 0.

- [ ] **Step 3: Delete the const and the `retain`; convert the nine scans**

Remove `const SCOPED_ROW_CHECKS` and the whole `if let Some(cp) = ctx.current_project.as_deref() { all_violations.retain(...) }` block at `:551-587`. Then in each of these scans, add `scope: &mut DoctorScope` and gate the row loop with `if !scope.admit(&abs_path) { continue; }`:

`scan_frontmatter_id_mismatches` (`:4611`), `scan_frontmatter_status_mismatches` (`:2478`), `scan_undefined_entries` (`:3744`, which emits both `ledger_defines_nothing` and `entry_without_definition`), `scan_entry_defined_twice` (`:3085`), `scan_terminal_status_with_caveat` (`:4653`), `scan_params_behind_body` (`:4308`), `scan_params_status_drift` (`:4518`), `scan_snapshot_drift` (`:3633`), `scan_augmentation_declared_but_absent` (`:4811`).

Prefer the `sql_and` splice where the scan owns its query — `scan_terminal_status_with_caveat` reads frontmatter off disk for every terminal bug row, so narrowing in SQL avoids those reads entirely.

- [ ] **Step 4: Run to verify it passes**

```
cargo test --workspace doctor::tests
```
Expected: PASS. `row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not` (`:11978`) will still pass — it asserts the asymmetry, which Task 7 changes, not this task.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/doctor/ && git commit -m "$(cat <<'EOF'
refactor(doctor): retire SCOPED_ROW_CHECKS; admit params_behind_body and params_status_drift

The seven-name string list becomes nine scans sharing one mechanism, closing
the gap the 2026-08-27 bug deferred: it left params_behind_body out because the
check reported zero, and instructed that it be added if it ever fired across
repos. Measured 2026-09-09 it fires 4 times, 2 of them foreign;
params_status_drift 3 times, 1 foreign.

Neither has a fix= mode, so scoping their report scopes nothing else.

Bug: d4b61746950b86b7

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Give the six silent scans a published tally

The scans at `:5152` (`scan_archived_fix_sha_unresolvable`), `:5342` (`scan_terminal_status_without_fix_anchor`), `:5577` (`scan_non_terminal_status_with_fix_anchor`), `:5779` (`scan_open_bug_cited_from_source`), `:5988` (`scan_unterminated_fence`) and `:6093` (`scan_claim_liveness`) each narrow with a bare `containing_root(std::slice::from_ref(&cp.git_root), path)` and report **nothing**. A reader cannot tell whether `terminal_status_without_fix_anchor: 9` is repo-local or machine-wide.

**Files:**
- Modify: `src/librarian/tools/doctor.rs` at the six sites above
- Test: `src/librarian/tools/doctor.rs` `mod tests`

**Interfaces:**
- Consumes: `DoctorScope::admit`. Produces: no new API — these six join the shared tally, so their drops appear in `row_checks_scoped_by_project`.

- [ ] **Step 1: Write the failing test**

```rust
/// These six scoped correctly and published no count — the one mechanism whose drop
/// was invisible. An observer asking "is this 9 mine or the machine's?" had no way to
/// answer from the report.
#[tokio::test]
async fn a_silently_scoped_check_now_reports_what_it_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mine");
    std::fs::create_dir_all(root.join("docs/issues")).unwrap();
    let ctx = ctx_at(&root);
    // A FOREIGN terminal bug file declaring no fix anchor.
    seed_terminal_bug_without_fix_anchor(&ctx, Path::new("/home/other/repo/docs/issues/b.md"));

    let v = call(&ctx, json!({ "scope": "project" })).await.unwrap();
    assert_eq!(v["summary"]["by_check"]["terminal_status_without_fix_anchor"], json!(0));
    assert!(
        v["catalog_health"]["row_checks_scoped_by_project"]
            .as_object().unwrap().values()
            .filter_map(|x| x.as_u64()).sum::<u64>() >= 1,
        "the drop must now be visible in the tally"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

```
cargo test --workspace doctor::tests::a_silently_scoped_check_now_reports_what_it_dropped
```
Expected: FAIL — the tally is 0 because these scans `continue` without counting.

- [ ] **Step 3: Convert the six sites**

At each, replace the guard. `scan_terminal_status_without_fix_anchor` (`:5347`) shows it:

```rust
// before
    let Some(cp) = ctx.current_project.as_deref() else { return Ok(Vec::new()) };
    // ... in the row loop:
    if super::containing_root(std::slice::from_ref(&cp.git_root), path).is_none() { continue; }

// after
    // ... in the row loop:
    if !scope.admit(abs_path) { continue; }
```

Two of these keep `ctx` for a *different* reason and must not lose it: `scan_archived_fix_sha_unresolvable` needs the repo to resolve a SHA against, and `scan_open_bug_cited_from_source` walks the source tree. Pass both `ctx` and `scope`.

- [ ] **Step 4: Run to verify it passes**

```
cargo test --workspace doctor::tests
```
Expected: PASS.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/doctor/ && git commit -m "$(cat <<'EOF'
fix(doctor): publish the drop for six scans that scoped silently

They narrowed correctly and reported no count, so a reader could not tell
whether terminal_status_without_fix_anchor: 9 was repo-local or machine-wide.
The author holds the scope parameter, so the gap is invisible from where they
sit — CLAUDE.md Observer Blindness.

Bug: d4b61746950b86b7

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Scope `fix=reseat_worktree`'s repair, then its report

The one documented exception. `doctor.rs:558-570` states it: *"`fix=reseat_worktree` only re-keys catalog rows. It takes no root and filters by none, reseating every unregistered worktree-scoped row in the catalog. Scoping its report while the repair stays machine-wide would understate what `confirm=true` is about to do."* The reasoning is sound, so the repair moves first.

**Files:**
- Modify: `src/librarian/tools/doctor.rs:1512-1573` (`reseat_worktree`), `:1198` (`run_fix`'s dispatch), `:2195` (`scan_worktree_scoped`)
- Test: `src/librarian/tools/doctor.rs` `mod tests` — including a deliberate rewrite of `:11978`

**Interfaces:**
- Consumes: `DoctorScope`. Produces: `reseat_worktree(ctx: &ToolContext, scope: &DoctorScope) -> Result<Value>`, whose report gains a `"scope"` field naming what it covered.

- [ ] **Step 1: Write the failing test, and rewrite the test that pins the old asymmetry**

`row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not` (`:11978`) asserts today's behaviour **as intended**. It must be rewritten, not deleted — its name becomes false and its second clause inverts:

```rust
/// Renamed from `row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not`.
/// The asymmetry it pinned was deliberate and is now closed: the repair takes a scope,
/// so the report may narrow without understating what `confirm=true` will do.
#[tokio::test]
async fn worktree_scoped_row_now_scopes_with_every_other_check() {
    let (_wt_tmp, main_root, worktree_root) = make_worktree_fixture();
    let ctx = ctx_at(&main_root);
    // A worktree-scoped row in ANOTHER repo entirely.
    seed_worktree_row(&ctx, Path::new("/home/other/repo/.worktrees/x/docs/t.md"));

    let project = call(&ctx, json!({ "scope": "project" })).await.unwrap();
    assert_eq!(
        project["summary"]["by_check"]["worktree_scoped_row"], json!(0),
        "another repo's worktree rows are not this developer's worklist"
    );
    let all = call(&ctx, json!({ "scope": "all" })).await.unwrap();
    assert!(all["summary"]["by_check"]["worktree_scoped_row"].as_u64().unwrap() >= 1);
    let _ = worktree_root;
}

/// The invariant that made the old asymmetry correct, now asserted directly: the
/// repair must never touch more than the report showed.
#[tokio::test]
async fn reseat_worktree_never_reseats_a_row_outside_its_scope() {
    let (_wt_tmp, main_root, _worktree_root) = make_worktree_fixture();
    let ctx = ctx_at(&main_root);
    let foreign = Path::new("/home/other/repo/.worktrees/x/docs/t.md");
    seed_worktree_row(&ctx, foreign);

    let v = call(&ctx, json!({ "fix": "reseat_worktree", "confirm": true })).await.unwrap();
    assert_eq!(
        v["reseated"].as_array().map(|a| a.len()).unwrap_or(0), 0,
        "a foreign worktree row must not be reseated by a project-scoped repair"
    );
    assert!(v["scope"].is_object(), "the repair must name the scope it covered");
}
```

- [ ] **Step 2: Run to verify they fail**

```
cargo test --workspace doctor::tests::worktree_scoped_row_now_scopes_with_every_other_check
cargo test --workspace doctor::tests::reseat_worktree_never_reseats_a_row_outside_its_scope
```
Expected: both FAIL — the report shows the foreign row, and the repair reseats it.

- [ ] **Step 3: Scope the repair, then the report**

In `run_fix`, build a `DoctorScope` for the active project and pass it to `reseat_worktree`. Inside, gate the candidate loop with `scope.contains(...)` and add `"scope": applied.to_json()` to the returned report. Then add `scope: &mut DoctorScope` to `scan_worktree_scoped` (`:2195`) and gate its row loop with `admit`.

Order matters: if the report is scoped first, there is a window where `confirm=true` does more than the report showed — the precise hazard the old comment refused. Do the repair in the same commit, repair first.

- [ ] **Step 4: Replace the stale comment**

Delete `doctor.rs:558-570`'s explanation of the exception and record the closure in the module doc instead, naming the invariant that replaced it (`reseat_worktree_never_reseats_a_row_outside_its_scope`).

- [ ] **Step 5: Run to verify they pass, and gate**

```
cargo test --workspace doctor::tests
cargo fmt && cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
```

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/doctor/ && git commit -m "$(cat <<'EOF'
fix(doctor): scope reseat_worktree's repair, then its report

The last documented scoping exception. Its reason was real — the repair took no
root and reseated every unregistered row in the catalog, so narrowing only the
report would understate what confirm=true was about to do. Repair scoped first,
report second, in that order, with the invariant now asserted directly rather
than encoded as an asymmetry.

Rewrites row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not,
which pinned the old behaviour as intended. Deliberate, not collateral.

Bug: d4b61746950b86b7

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: The cited-from-here relevance rule, shipped with the scope of its own zero

**Read this before implementing it.** The rule — *a foreign finding surfaces iff a local artifact cites it* — is **inert on today's catalog**, and measured so:

- 6 of the 15 foreign findings checked have **zero** inbound `cites` edges (`690f130645515497`, `8d93149691eb85fe`, `72843ff4a157b6aa`, `acfaaf4269d8d3fb`, `626ce2d3fa74dd7c`, `fb7592ff7c344255`). Every edge found is outbound and intra-repo.
- Structurally: `link_scan` builds both `DefinitionIndex` and `Corpus` from `rows`, which come from `cat_find::find(filter: scoped_filter)` (`link_scan/mod.rs:352-400`), and its default scope is `Project` (`:315`). So the automatic derivation path **cannot** write a local→foreign edge.
- The explicit path can: `append_entry(cites=[…])` resolves refs corpus-wide. So the edge is *representable but unpopulated*.

So this task must not ship a predicate whose failure no observer would see. It ships the predicate **and** the denominator, per `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.

**Files:**
- Modify: `src/librarian/tools/doctor/scope.rs` (add `cited_from_scope`), `src/librarian/tools/doctor.rs` (`call`'s hint assembly)
- Test: `src/librarian/tools/doctor/scope.rs` `mod tests`

**Interfaces:**
- Consumes: `DoctorScope::contains`; the `links` table (`rel = 'cites'`).
- Produces: `DoctorScope::admit` gains the exemption; `catalog_health` gains `cross_root_cites_edges: usize` and a hint naming it.

- [ ] **Step 1: Write the failing tests**

```rust
/// The rule itself: a foreign row a LOCAL artifact cites is this developer's problem.
#[test]
fn a_foreign_row_cited_from_the_active_project_is_admitted() {
    let cat = Catalog::open_in_memory().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mine");
    std::fs::create_dir_all(root.join("docs")).unwrap();
    let local = root.join("docs/mine.md");
    let foreign = std::path::PathBuf::from("/home/other/repo/docs/theirs.md");
    let (local_id, foreign_id) = (insert_artifact_row(&cat.conn, &local), insert_artifact_row(&cat.conn, &foreign));
    insert_cites_edge(&cat.conn, &local_id, &foreign_id);

    let ctx = ctx_at_with_catalog(&root, cat);
    let mut s = DoctorScope::new(Scope::Project, &ctx).unwrap();
    assert!(
        s.admit(&foreign.to_string_lossy()),
        "a foreign artifact this project cites affects this project"
    );
    assert!(s.scoped_out().is_empty(), "an admitted row must not also be tallied as dropped");
}

/// The denominator. Without this the rule is decoration: it is satisfied by a catalog
/// with no cross-root edges at all, which is exactly today's catalog, and a reader
/// cannot tell "nothing is relevant" from "relevance was never computable".
#[test]
fn the_cross_root_edge_count_is_published_even_when_zero() {
    let cat = Catalog::open_in_memory().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mine");
    std::fs::create_dir_all(&root).unwrap();
    let ctx = ctx_at_with_catalog(&root, cat);
    let s = DoctorScope::new(Scope::Project, &ctx).unwrap();
    assert_eq!(
        s.cross_root_cites_edges(), 0,
        "an empty catalog has none — and the report must SAY so rather than omit it"
    );
}
```

- [ ] **Step 2: Run to verify they fail**

```
cargo test --workspace doctor::scope::tests::a_foreign_row_cited_from_the_active_project_is_admitted
cargo test --workspace doctor::scope::tests::the_cross_root_edge_count_is_published_even_when_zero
```
Expected: FAIL to compile — `insert_cites_edge`, `ctx_at_with_catalog` and `cross_root_cites_edges` do not exist.

- [ ] **Step 3: Implement the exemption and the counter**

In `DoctorScope::new`, after `roots` is built, query the `cites` edges whose source is in scope and whose destination is not, storing the destination ids in a `BTreeSet<String>` plus the count. In `admit`, admit a foreign row whose id is in that set. `admit` therefore needs the artifact **id**, not only the path — change its signature to `admit(&mut self, id: &str, abs_path: &str) -> bool` and update every call site from Tasks 3–6.

- [ ] **Step 4: Publish the denominator in the hint**

Where `call` assembles `catalog_health.hint`, append a sentence built from the counter. When it is zero the wording must name the remedy, not merely the number:

```rust
    if scope.cross_root_cites_edges() == 0 {
        hint.push_str(
            " Cross-project relevance was computed over 0 `cites` edges crossing a root \
             boundary, so NO foreign finding could have been admitted by it — this is not \
             evidence that none is relevant. link_scan writes edges only within its own \
             scope, so run librarian(action=\"link_scan\", scope=\"umbrella\", write=true) to \
             populate them, or cite deliberately with append_entry(cites=[…]).",
        );
    }
```

- [ ] **Step 5: Run to verify they pass, and gate**

```
cargo test --workspace doctor::scope::tests
cargo fmt && cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
```

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/doctor/ && git commit -m "$(cat <<'EOF'
feat(doctor): admit a foreign finding the active project cites, and publish the denominator

The relevance rule for "cross-project only if it affects the current project".
It is INERT on today's catalog and measured so: 6 of 6 foreign findings checked
have zero inbound cites edges, and link_scan builds its definition index from
scope-filtered rows so the automatic path cannot write a local->foreign edge at
its default project scope.

So the predicate ships with its scope: catalog_health reports how many
cross-root cites edges the rule was computed over, and says what to run when
that is zero. A rule whose failure no observer would notice is decoration
however loudly written.

Bug: d4b61746950b86b7

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: Collapse the 112-root inventory at non-`all` scope

`catalog_health.outside_roots_by_project` enumerates every foreign project root by absolute path — 112 of them, ~114 lines of a 316-line health block, including paths like `/home/marius/agents/system/old-ubuntu-config/crash-history`. Ruling 17 requires the **metric** to stay global, and a total plus a top-N satisfies that while a full inventory is what a reader actually notices.

**Files:**
- Modify: `src/librarian/tools/doctor.rs:~770` (the `outside_by_project` assembly)
- Test: `src/librarian/tools/doctor.rs` `mod tests`

**Interfaces:** Consumes `DoctorScope::scope`. Produces: at `Scope::All` the map is unchanged; below it, `outside_roots_by_project` carries the top 10 roots by count plus `{"_other_roots": N, "_other_rows": M}`, and a new sibling `outside_roots_total` carries the full sum.

- [ ] **Step 1: Write the failing test**

```rust
/// The metric must survive the collapse — a total that shrank with the display would
/// be the false negative Ruling 17 forbids.
#[tokio::test]
async fn the_outside_roots_inventory_collapses_but_its_total_does_not() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mine");
    std::fs::create_dir_all(&root).unwrap();
    let ctx = ctx_at(&root);
    for i in 0..25 {
        insert_artifact_row(&ctx.catalog.lock().conn, Path::new(&format!("/home/r{i}/docs/x.md")));
    }

    let project = call(&ctx, json!({ "scope": "project" })).await.unwrap();
    let shown = project["catalog_health"]["outside_roots_by_project"].as_object().unwrap();
    assert!(shown.len() <= 12, "the inventory must collapse; got {} keys", shown.len());
    assert_eq!(
        project["catalog_health"]["outside_roots_total"], json!(25),
        "the global total must be unchanged by the collapse"
    );

    let all = call(&ctx, json!({ "scope": "all" })).await.unwrap();
    assert_eq!(
        all["catalog_health"]["outside_roots_by_project"].as_object().unwrap().len(), 25,
        "scope=all asked for the whole picture and must still get it"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

```
cargo test --workspace doctor::tests::the_outside_roots_inventory_collapses_but_its_total_does_not
```
Expected: FAIL — 25 keys at project scope, and `outside_roots_total` absent.

- [ ] **Step 3: Implement the collapse**

Sort the assembled map by count descending, take 10, and fold the tail into `_other_roots`/`_other_rows`. Emit the full map only when `scope.scope == Scope::All`. Add `outside_roots_total` unconditionally.

- [ ] **Step 4: Run to verify it passes, gate, commit**

```bash
cargo test --workspace doctor::tests
cargo fmt && cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/doctor/ && git commit -m "$(cat <<'EOF'
feat(doctor): collapse the outside-roots inventory below scope=all

112 foreign project roots enumerated by absolute path were ~114 lines of a
316-line health block. Ruling 17 needs the metric, not the inventory: the total
ships unconditionally as outside_roots_total, the top 10 roots stay, the tail
folds into _other_roots/_other_rows, and scope=all still returns everything.

Bug: d4b61746950b86b7

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: Sweep the prompt surfaces and guides, then archive the bug

**Files:**
- Modify: `src/prompts/source.md` (if it names `doctor`'s scope behaviour), `.codescout/system-prompt.md`, `src/librarian/tools/guide_*` doctor sections, `CLAUDE.md` § *Querying active trackers* if it describes doctor's scope
- Modify: `docs/issues/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`

- [ ] **Step 1: Find every surface that describes doctor's scope**

```
grep -rn 'doctor' src/prompts/ .codescout/system-prompt.md docs/conventions/ CLAUDE.md
```

`.codescout/system-prompt.md` is a **fourth prompt surface**, tracked in git and injected into every session in this repo — sweep it whenever the other three move. Regenerate with `onboarding(refresh_prompt=true)` if it drifts.

- [ ] **Step 2: Update `get_guide("librarian")`'s doctor rows**

The § *librarian(action=…) — Reference* row for `doctor` and § *doctor repairs* must state that the report is project-scoped by default and that `fix=reseat_worktree` now takes a scope.

- [ ] **Step 3: Run the prompt-surface gate**

```
cargo test --workspace prompt_surfaces_reference_only_real_tools
cargo test --workspace claude_md_contains_no_deprecated_tool_names
cargo test --workspace claude_md_gate_lists_its_four_commands_in_the_load_bearing_order
```
Expected: PASS. The third pins CLAUDE.md's gate sentence byte-for-byte — if you touched it, move that test with it rather than deleting it.

- [ ] **Step 4: Record the fix pair and archive**

Both identifiers, because they fail differently:

```bash
git log -1 --format=%H                      # the SHA, on experiments
git show <sha> | git patch-id --stable      # content hash, survives rebase
```

Write both into the bug file's § *Fix*, then archive through the librarian — never a bare `git mv`, which orphans the catalog row:

```
doc(action="update", id="d4b61746950b86b7", patch={"status": "fixed", "extra": {"closed": "2026-09-XX"}})
doc(action="move", id="d4b61746950b86b7", new_rel_path="docs/issues/archive/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md")
```

`doc(action="move")` **mints a new id** (`id = sha256(abs_path)`) and grafts the history; read `id`/`previous_id` from the response and do not reuse the cached id.

- [ ] **Step 5: Re-measure the title's own number before archiving**

Required by `bug-fix-session-log:F-75`, which a predecessor of this very bug generated: *a fix verified against its mechanism can leave its title's number untrue.* The title claims "under-reports by 636 rows". Re-run `librarian(action="doctor", scope="all")` against the rebuilt binary and state the figure that holds at archive time, even if it moved. Stash-measure-restore, or the corpus moves under the measurement.

- [ ] **Step 6: Close the ledger entries**

Flip `F-125` to `promoted-to-bug-tracker` and `W-117` to `validated` (already) via `doc(action="update", …)` with a `body_edits` patch — never `patch={params:…}`, whose array semantics replace the whole collection.

---

## Self-Review

**1. Spec coverage.** The bug's § *Fix* lists six items: typed `Args` + probe exemption (Task 1), `resolve_scope` with `Project`/`Require` (Task 1), SQL-layer application (Task 2), echoing scope and fallback (Task 1), re-graining `git_root`→`abs_path` (Task 2's `DoctorScope::new`, applied at every converted site in Tasks 3–6), Ruling 17 preservation (asserted in Tasks 2, 3, 8). § *Tests added* lists four planned test names; all four appear, with `doctor_scope_all_reports_more_than_scope_project` renamed to `scope_all_reports_strictly_more_than_scope_project` to put the strict inequality in the name. The three design decisions are covered: scope grain (Task 1–2), `reseat_worktree` repair-then-report (Task 6), cited-from-here relevance (Task 7). **Gap accepted:** `a06de4dfc30c2e8d` (CLI empty args) is deliberately out of scope and stated as such in the header.

**2. Placeholder scan.** No `TBD`/`TODO`/"similar to Task N". Three places direct the implementer to *read a named existing site before writing* rather than giving code: `TestToolContextBuilder`'s real methods (Task 1 Step 2), `apply_scope`'s `Scope::Umbrella` arm (Task 2 Step 3), and the validity-fixture body (Task 3 Step 1). These are deliberate — each is a case where a guessed shape compiles and silently tests the wrong thing, so the instruction is to copy the existing form. Each names the exact file, symbol and line.

**3. Type consistency.** `DoctorScope::admit` changes signature in Task 7 (`admit(&mut self, abs_path)` → `admit(&mut self, id, abs_path)`); Task 7 Step 3 states that every call site from Tasks 3–6 must be updated, so the change is not silent. `Scope`, `UmbrellaPolicy`, `resolve_scope`, `apply_scope`, `SqlFragment`, `compile`, `containing_root`, `CurrentProject` are all used with the signatures read from source this session. `scoped_out()` returns `&BTreeMap<String, usize>` and is cloned at the one call site that needs ownership.

**One risk the plan does not remove.** Tasks 3–6 convert ~20 scan functions. Each conversion is mechanical, but `doctor.rs` is ~15,500 lines with ~280 tests, and several tests construct scans directly rather than through `call`. Expect per-task test-fixture churn, and treat a red in a test you did not name as a finding to read rather than a fixture to adjust.

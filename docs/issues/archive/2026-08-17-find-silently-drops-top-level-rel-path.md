---
id: '7d9e2dc48eb2b128'
kind: bug
status: fixed
title: 'BUG: artifact(find) silently drops a top-level `rel_path` and returns page 1 of the catalog as `count: 50` — the number that reads as a match total is the default limit'
tags:
- librarian
- artifact-tool
- find
- schema
- silent-drop
- cluster/accepted-parameter-silently-dropped
closed: 2026-08-17
opened: 2026-08-17
owner: marius
related:
- docs/issues/archive/2026-07-20-artifact-update-toplevel-status-param-silently-dropped.md
- docs/issues/archive/2026-07-13-artifact-update-phantom-schema-fields.md
- docs/issues/archive/2026-07-10-artifact-filter-inversion-misleading-hint.md
severity: medium
---

## Summary

`artifact(action="find", rel_path="<path>")` ignores `rel_path` entirely and returns the
first 50 artifacts in the catalog under `count: 50`. `rel_path` **is** an advertised
top-level param and its description is written partly in `find` terms, but `find::Args`
has no such field and no `#[serde(deny_unknown_fields)]` — so serde drops the key and the
call runs at defaults: `filter: None`, `limit: 50`.

The response is well-formed, contains 50 real artifacts, and carries no error, warning or
`corrections` block. That is what makes it dangerous: **the count that reads as a match
total is the default page size.**

## Symptom (Effect)

Two live calls, same intent, 2026-08-17 on `experiments` at `637b9d37`:

```
artifact(action="find", rel_path="docs/trackers/open-issue-work-queue.md")
→ count: 50
  items: 0078deac99b4d2e8 [fixed] BUG: artifact(find) answers from the catalog…
         e7454abe1bfe5784 [fixed] BUG: a heading concatenated onto the end…
         7e498b6dcb45b924 [active] Tracker hygiene log
         db1145e76b0cbda2 [unknown] TAXONOMY — ID prefixes used in this repo
         … +42 more — narrow the filter, or read them from the buffer
```

None of the 50 is the requested file. The same question as a filter:

```
artifact(action="find", filter={"rel_path": {"contains": "open-issue-work-queue"}})
→ count: 1
  items: a742a50ea6723daf … docs/trackers/open-issue-work-queue.md
```

The discriminator is the pair, not either call alone: `50` vs `1` on identical intent.

## Reproduction

Branch `experiments`, HEAD `637b9d37`. Live MCP (`cargo rb` + `/mcp`).

1. `artifact(action="find", rel_path="docs/trackers/open-issue-work-queue.md")`
2. Observe `count: 50`, no error, no `corrections`, and items unrelated to the path.
3. `artifact(action="find", filter={"rel_path": {"contains": "open-issue-work-queue"}})`
4. Observe `count: 1` and the correct row.

Any catalog with >50 artifacts reproduces it. With fewer than 50 the bug is *more*
deceptive, not less: the count then equals the catalog size rather than the page size, so
the round-number tell disappears.

## Environment

codescout MCP server, `artifact` tool, `find` action. Rust, in-process SQLite catalog.
Branch `experiments`, project `codescout`. Session resumed post-compaction (irrelevant to
the mechanism; noted only because it is when the call was made).

## Root cause

Measured 2026-08-17: the two calls above, plus the schema and `Args` read at `637b9d37`.

**1. The param is dropped by serde.** `find::Args`
(`src/librarian/tools/find.rs:16-45`) declares exactly `filter`, `kind`, `status`,
`limit`, `offset`, `semantic`, `scope`, `include_archived`, `augmented`. No `rel_path`,
and the struct carries no `#[serde(deny_unknown_fields)]`, so an unknown top-level key is
discarded on deserialize. `limit` then defaults to `50` via `default_limit()`
(`src/librarian/tools/find.rs:47-49`) and `filter` to `None` — an unfiltered first page.

**2. The schema invites the call.** `rel_path` is declared as a top-level property
(`src/librarian/tools/artifact.rs:105`). Its description opens with the `create:` label
and then spends two further sentences on a different action:

> `create: relative path for new file. In find results: path relative to repo root — does
> NOT include the repo name (use the `repo` field for that). When filtering by path use
> contains/prefix on the path portion only, e.g. {"contains": {"field": "rel_path",
> "value": "docs/trackers"}}.`

Of the 37 action-labelled properties in that schema, this is the one whose prose crosses
actions. An agent looking for "how do I find by path" finds `rel_path` — at top level,
described in find terms.

**3. The example in that description teaches a known-wrong shape.** The
`{"contains": {"field": …, "value": …}}` form is the *inverted* leaf, which
`repair_node` (`src/librarian/filter.rs:243-250`) exists to correct and whose comment
calls it *"the most common filter error"* per `usage.db`. Verified on the wire — it runs
and self-reports:

```
artifact(action="find", filter={"contains": {"field": "rel_path", "value": "codescout-usage-frictions"}})
→ count: 1
  corrections.filter: ["inverted filter leaf repaired: {\"contains\":{\"field\":\"rel_path\",…}}
                        → {\"rel_path\":{\"contains\":…}}"]
  corrections.hint: "Filter leaf shape is {field: {op: value}}, not {op: {field, value}}…"
```

So the schema propagates the error its own repair path absorbs.

**The asymmetry is the defect, stated precisely.** `find` already has a
Repair-and-Continue surface for caller error: a *malformed* filter is repaired, reported,
and taught. A *missing* filter — because the key naming it was silently dropped — gets
nothing. The louder mistake gets the help; the silent one gets a confidently wrong answer.

## Evidence

### The `Args` struct at HEAD

```
$ git show HEAD:src/librarian/tools/find.rs   (lines 15-49)
#[derive(Deserialize)]
struct Args {
    #[serde(default)] filter: Option<FilterNode>,
    #[serde(default)] kind: Option<String>,
    #[serde(default)] status: Option<String>,
    #[serde(default = "default_limit")] limit: usize,
    #[serde(default)] offset: usize,
    #[serde(default)] semantic: Option<String>,
    #[serde(default)] scope: Option<Scope>,
    #[serde(default)] include_archived: bool,
    #[serde(default)] augmented: Option<bool>,
}
fn default_limit() -> usize { 50 }
```

No `rel_path`; no `deny_unknown_fields`.

### Every action-labelled schema property, and its label

Extracted from `input_schema()` at HEAD (37 properties). Every one carries a single
action label except `rel_path`, whose body then discusses `find`:

```
find:         filter kind status semantic scope augmented
get:          include_links links_direction links_rel full heading items
              entry_filter start_line end_line
move:         new_rel_path
create:       rel_path repo title body augment
update:       force commit_refresh
link:         src_id dst_id rel
graph:        depth items include_events
state_at:     artifact_id commit timestamp
update_entry: entry_id fields
append_entry: id_prefix entry items
```

### Why `deny_unknown_fields` is not the fix

`src/librarian/tools/artifact.rs:262-269`, the regression test guarding an earlier
attempt:

```rust
async fn update_action_passes_through_dispatcher_without_unknown_field_error() {
    // Regression: deny_unknown_fields on update::Args used to reject the
    // outer dispatcher's `action` field, breaking every artifact(update)
    // call through the Tool surface.
```

and `src/librarian/tools/update.rs:295-302`:

> *"`Args` cannot carry `deny_unknown_fields` — the dispatcher passes `action` through and
> the shared artifact schema carries create-only keys — so any advertised param *missing*
> from `Args` is discarded by serde while the call still returns `updated: true`."*

## Hypotheses tried

1. **Hypothesis:** `rel_path` is a recognized `find` filter shortcut (like `kind` and
   `status`) that is mis-comparing absolute vs displayed paths — i.e. this is U-35 again.
   **Test:** read `find::Args` and `merge_kind_status` at HEAD.
   **Verdict:** rejected. Only `kind` and `status` have shortcut expansion; `rel_path` has
   no field at all, so nothing compares anything. U-35 is a genuinely different defect
   (`eq` vs `contains` *inside* a recognized filter).
   **Evidence link:** *The `Args` struct at HEAD*.

2. **Hypothesis:** already covered by
   `2026-07-13-artifact-update-phantom-schema-fields.md`, whose fix deleted schema keys
   with no backing and added `input_schema_has_no_phantom_update_fields`.
   **Test:** read that bug in full.
   **Verdict:** rejected — and the reason is the useful part. Those keys were backed
   *nowhere*, so deletion was correct and the test catches recurrence. `rel_path` **is**
   legitimately backed — by `create`. The existing test asserts a key is honored by *some*
   action; nothing asserts it is honored by the action whose description claims it. This
   is the variant that test is structurally unable to catch.
   **Evidence link:** *Every action-labelled schema property, and its label*.

3. **Hypothesis:** the inverted-leaf example in `rel_path`'s description is simply broken
   and would error.
   **Test:** ran it live.
   **Verdict:** rejected — `repair_node` fixes it and reports the correction. The defect is
   subtler than "broken example": the schema *teaches* the error the repair path exists to
   absorb, spending a repair on every caller who follows the docs.
   **Evidence link:** Root cause §3.

## Fix

**Fixed on `experiments`, commit `4fad1aa4`** — this is the SHA to cite.

Cherry-picked from `0a955491` on `worktree-il3-gate-and-find-lift` (pushed to `origin`).
A fast-forward was **not** available: the two had diverged by one commit each way, because
the bookkeeping commit that recorded this fix (`f273f187`) landed on `experiments` after
the branch was cut. Per the after-cherry-pick rule the branch-side original orphans on the
next rebase, so `0a955491` is history and `4fad1aa4` is the citation.

All three planned steps shipped.

**1. Lift, don't reject.** `find::Args` gains `rel_path: Option<String>`, and `call()`
lifts it into `filter={"rel_path": {"contains": v}}`, reporting the rewrite under
`corrections` — the `lift_top_level_param!` pattern from
`src/librarian/tools/update.rs`, which exists because this class shipped twice on
`update`. `contains`, not `eq` (U-35).

Two placement details that are not incidental:

- The lift runs **before `is_cold_call`**, so a `rel_path` call correctly stops counting
  as a cold call — it is a filtered query.
- It also runs **before `rel_path_hint`**, so it inherits, for free, the existing
  behaviour where an empty page from a `rel_path` filter triggers a disk scan for
  matching-but-unindexed files. A caller who searches by path and gets nothing now learns
  whether the file exists but is not indexed.

The `corrections.hint` is composed per repair rather than being one fixed string: a lift
reported under the inverted-leaf hint would tell the caller to fix a filter shape they
never wrote.

**2. Split the description.** `rel_path`'s schema entry keeps its `create:` label, states
the find shorthand it now genuinely supports, and **drops the inverted-leaf example** that
was teaching the shape `repair_node` exists to correct — the one its own comment calls the
most common filter error.

**3. Close the class.** `schema_keys_labelled_find_are_honored_by_find` — see *Tests
added*.
## Tests added

**`src/librarian/tools/find.rs`** — four, all watched RED first:

- `lifts_top_level_rel_path_into_a_contains_filter` — RED at `count: 2` (the unfiltered
  page) against an expected 1.
- `reports_the_lifted_rel_path_under_corrections` — a silent lift is the same defect in a
  new costume, so the report is asserted by name.
- `lifted_rel_path_uses_contains_so_a_displayed_path_still_matches` — the U-35 interaction.
- `lifted_rel_path_combines_with_an_explicit_filter` — the lift ANDs into an explicit
  filter rather than replacing it, asserted in both directions (both clauses apply; the
  `rel_path` clause actually narrows inside the AND).

**`src/librarian/tools/artifact.rs`** — two:

- `schema_keys_labelled_find_are_honored_by_find` — the class-level guard, and the durable
  half of this fix. It **passes today**, so it is a tripwire for the next variant rather
  than a reproduction of this one; **mutation-verified** by adding a `find:`-labelled
  schema key with no backing field, which makes it fail naming that key. The probe uses
  serde's asymmetry: a key that IS a field type-checks and rejects `[]`, a key that is not
  is silently discarded and the call succeeds. `[]` is invalid for every type in
  `find::Args`, which is noted in the test because the probe is unsound for an `Args`
  holding a `Vec`.
- `rel_path_description_and_find_support_agree` — the doc half. The invariant is **not**
  "never mention another action": mentioning `find` is now correct, because `find` honors
  the key. It is that the mention and the support must agree, which was false before and is
  true now.

**Two of these tests were vacuous passes before being corrected**, and both are recorded in
their own comments:

- `schema_keys_labelled_find_are_honored_by_find` cannot reach `rel_path` at all — the key
  is labelled `create:`, so a label-driven sweep misses it *by construction*. That is
  precisely why the doc-half test exists separately, and it is the sharpest statement of
  the bug: the defect lives in the gap between a key's label and its prose.
- `lifted_rel_path_uses_contains_…` first seeded one row, so `count == 1` held whether or
  not `rel_path` was honored.

A third near-miss is worth recording here because it is the same failure mode one level up:
the first version of this test file's sibling assertion searched the description for
`{"contains"`, which also matches the **canonical** `{"rel_path": {"contains": …}}`. The
inverted shape's real signature is the `"field"` key.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test`
**4069 passed, 0 failed, 45 ignored**.
## Workarounds

Use the filter form, with `contains` on a path fragment:

```
artifact(action="find", filter={"rel_path": {"contains": "open-issue-work-queue"}})
```

`eq` will return `0` for the path as displayed in responses — the catalog stores absolute
paths and the relative form is a display-time transform (U-35 in
`docs/trackers/codescout-usage-frictions.md`).

**Detection tell, for any `find` result:** a `count` exactly equal to the default limit of
50, with items that do not match the intent, means the filter never ran. Re-read the call
before the results.

## Resume

N/A — closed. Fixed on `experiments` at **`4fad1aa4`** and archived 2026-08-17 after wire
verification.

**No pending-master-SHA line, deliberately.** `git rev-list --left-right --count
master...experiments` returns `0 955` — a zero on the left means `master` is a strict
ancestor, so the promotion path is **fast-forward**: `master` moves onto this exact commit
and `4fad1aa4` already *is* the master SHA. The line would send a later session hunting a
SHA that will never exist. (The cherry-pick recorded in *Fix* was branch → `experiments`,
a level below, and did mint a new SHA — hence the re-point from `0a955491`.)

**Wire-verified after `cargo rb` + `/mcp`:**

```
artifact(action="find", rel_path="docs/trackers/open-issue-work-queue.md")
→ count: 1   (was 50)
  corrections.filter: ["top-level `rel_path` lifted into the filter:
                       {\"rel_path\": {\"contains\": \"docs/trackers/open-issue-work-queue.md\"}}"]
  corrections.hint:   "`rel_path` is a create-time param; on find it was read as a filter
                       clause. Pass filter={\"rel_path\": {\"contains\": …}} directly next time."
```

Two follow-ups this fix deliberately did **not** take on, both still open:

1. **Extend the parity probe to the other actions.** Scoped to `find` because that is where
   the defect was measured. Each action needs a probe value ill-typed for *its* `Args` (`[]`
   is unsound where a `Vec` field exists) plus its required params, so the only possible
   error is the type error. The label extraction from `input_schema()` is already written.
2. **`repo`** carries a `create:` label and is the nearest sibling in shape to `rel_path` —
   worth one look for the same label-versus-prose mismatch.
## References

- `docs/trackers/codescout-usage-frictions.md` — U-42 (this friction), U-35 (`rel_path`
  `eq` vs `contains`).
- `docs/trackers/reconnaissance-patterns.md` — R-104: a zero from a report is a claim
  about your query. This bug is the non-zero case of the same lesson.
- Same class, archived: `docs/issues/archive/2026-07-20-artifact-update-toplevel-status-param-silently-dropped.md`,
  `docs/issues/archive/2026-07-13-artifact-create-drops-topic.md`,
  `docs/issues/archive/2026-07-13-artifact-update-phantom-schema-fields.md`,
  `docs/issues/archive/2026-07-10-artifact-filter-inversion-misleading-hint.md`.
- `src/librarian/tools/find.rs`, `src/librarian/tools/artifact.rs`,
  `src/librarian/tools/update.rs`, `src/librarian/filter.rs`.

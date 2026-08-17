---
id: '674359c8d4396147'
kind: bug
status: open
title: 'BUG: artifact(find) silently drops a top-level `rel_path` and returns page 1 of the catalog as `count: 50` — the number that reads as a match total is the default limit'
tags:
- librarian
- artifact-tool
- find
- schema
- silent-drop
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

Not yet implemented. Plan, in the order the codebase's own precedent suggests:

1. **Lift, don't reject.** Accept `rel_path: Option<String>` on `find::Args` and lift it
   into `filter={"rel_path":{"contains": v}}`, reporting the rewrite under `corrections`
   exactly as the inverted-leaf repair already does. This is the `lift_top_level_param!`
   pattern (`src/librarian/tools/update.rs:289-303`), which exists because this class
   shipped twice on `update`. Repair is unambiguously safe here in a way it is not on a
   write: `find` is a read, and there is exactly one sensible reading of a path-valued
   `rel_path` on a query. Use `contains`, not `eq` — `eq` compares against the stored
   absolute path (U-35).
2. **Split the description.** Move the find guidance off `rel_path` and onto `filter`,
   and delete the inverted example rather than leaving the schema teaching the error
   `repair_node` corrects.
3. **Close the class, not the instance.** Add a schema-hygiene test asserting that for
   every property whose description is labelled `<action>:`, that action's `Args` has a
   field of that name (or an explicit, named allowlist entry saying why not). That is the
   guard the family has been missing through four separate bugs; `rel_path` is simply the
   first one where the key was real.

## Tests added

None yet — the fix is unimplemented. Planned, with the failure each must show first:

- `find_lifts_top_level_rel_path_into_a_contains_filter` — RED today: returns the
  unfiltered page. Must go green returning the single matching row.
- `find_reports_the_lifted_rel_path_under_corrections` — a lift the caller cannot see is
  the same silent behavior in a new costume; assert the `corrections` key by name.
- `every_action_labelled_schema_key_is_honored_by_that_action` — the class-level parity
  test. RED today on `rel_path` alone; mutation-verify by re-labelling another property's
  description and confirming it fails.

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

Implement step 1 in `src/librarian/tools/find.rs`: add `rel_path: Option<String>` to
`Args`, and in `call()` fold it into the filter alongside `merge_kind_status`, appending a
`corrections.filter` note in the same shape `repair_node` emits. Write
`find_lifts_top_level_rel_path_into_a_contains_filter` first and watch it fail with the
50-row page. Then step 3, which is where the durable value is — the parity test over
`input_schema()` is what stops variant five.

Note `src/librarian/tools/find.rs` was uncommitted in a concurrent session's working tree
on 2026-08-17; re-check `git status` and rebase onto their work before editing it.

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


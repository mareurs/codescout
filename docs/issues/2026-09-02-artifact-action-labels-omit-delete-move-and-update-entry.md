---
status: open
opened: 2026-09-02
closed:
severity: low
owner: marius
related:
  - docs/issues/archive/2026-09-01-graft-requires-two-params-the-schema-never-advertises.md
tags:
  - cluster/doc-contradicted-by-code
kind: bug
---

# BUG: `artifact`'s action labels omit `delete`, `move` and `update_entry` for `id`, so `delete` has no labelled param at all

## Summary

`artifact` is a twelve-action dispatcher whose 55 flat params say which action they serve
through a prose prefix — `find:`, `get:`, `append_entry:` … That grammar is the only thing a
reader has, and it is incomplete where it matters most: `id` is labelled
`get/update/graph/append_entry` while `delete`, `move` and `update_entry` each *require* it.
Read off the wire, `delete` has zero params labelled for it and `move` has one (`new_rel_path`,
not the id it moves). Four params carry no description at all, one of them (`include_observations`)
a `get` option nothing attributes to any action, and `state_at` alone calls the artifact
`artifact_id`. The honesty of the labels is tested; their completeness is not.

## Symptom (Effect)

Wire (`tools/list`, 2026-09-02), `artifact.properties`:

```
id:                   "get/update/graph/append_entry: artifact id"
artifact_id:          "state_at: artifact id"
include_observations: {type: boolean, default: false}          ← no description
include_archived:     {type: boolean, default: false}          ← no description
limit:                {type: integer, default: 50, maximum: 500} ← no description
offset:               {type: integer, default: 0, maximum: 100000} ← no description
```

Labelled-params-per-action, computed over the dump (label = tokens before the first `:`):

```
find 6 · get 11 · create 10 · update 9 · move 1 · delete 0 · graft 2 · link 3
graph 4 · state_at 3 · append_entry 6 · update_entry 3
UNLABELLED: include_archived, limit, offset, include_observations
```

## Reproduction

`git rev-parse HEAD` → `4dc0daa2`. `python3 scripts/probe_tool_surface.py --json`; read
`artifact.inputSchema.properties.id.description`. Then:

```
grep -n "id: String" src/librarian/tools/{delete,mv,update_entry,get,update,graph,append_entry}.rs
```

Seven `Args` structs require `id`; the label names four.

## Environment

Not environment-dependent.

## Root cause

- `src/librarian/tools/artifact.rs:79` — `id`'s description. Introduced with the label
  `get/update/graph/append_entry` in `a7355498` (2026-07-06, the commit that added
  `append_entry`). `delete.rs` (`c40d5cbe`, 2026-05-29) and `mv.rs` (`8887fec6`, 2026-05-03)
  already required `id` then — so for those two the label was **incomplete from birth**, not
  decayed; `update_entry.rs` (`02a87a83`, 2026-08-16) arrived after and was not added. The
  class fit is on the mechanism, not the timeline: prose about code, and no check that reads
  one against the other.
- `src/librarian/tools/artifact.rs:90` — `include_observations`, consumed at
  `src/librarian/tools/get.rs:187` and `:242`; never described or labelled.
- `src/librarian/tools/artifact.rs:169` — `artifact_id` for `state_at`
  (`src/librarian/tools/state_at.rs:149`, no `#[serde(alias = "id")]`).
- Gates, and their direction:
  `every_action_labelled_schema_key_is_honored_by_that_action` (`src/librarian/tools/artifact.rs:361`) proves
  **labelled ⇒ honoured**; `assert_required_are_advertised` (`src/librarian/tools/mod.rs:578`)
  proves **required ⇒ advertised**. Neither proves **honoured ⇒ labelled** or
  **advertised ⇒ described**. Both are the monotone direction `CLAUDE.md` § *Testing Discipline*
  names: a label can be dropped or never written and every existing test stays green.

Measured 2026-09-02: the wire dump; the `git log` dates above; the per-action count.

## Evidence

### Wire
`tools_list.json` and `per_tool.json` in the session scratchpad,
`/tmp/claude-1000/-home-marius-work-claude-codescout/2cb44cd3-8673-4604-a8ac-5adea75ca54b/`.

### Args structs requiring `id`
`delete.rs:10`, `mv.rs:11`, `update_entry.rs:9`, `get.rs:185`, `update.rs:88`, `graph.rs:13`,
`append_entry.rs:9`; `state_at.rs:149` requires `artifact_id`.

### Sibling on the same tool, already archived
`2026-09-01-graft-requires-two-params-the-schema-never-advertises` — `from_id`/`into_id` were
required and absent; fixed `6894b67d`. That was **required-but-unadvertised**; this is
**advertised-but-unlabelled / undescribed**. Same tool, same grammar, the other gap.

## Hypotheses tried

1. **Hypothesis:** `delete`/`move` take the id under another key the label does cover.
   **Test:** `Args` structs. **Verdict:** rejected — plain `id: String`.
2. **Hypothesis:** the label lists only actions where `id` is *optional*, as a convention.
   **Test:** `get`, `update`, `graph`, `append_entry` all require it. **Verdict:** rejected.

## Fix

Plan, not implemented.

1. `id`: label `get/update/move/delete/graph/append_entry/update_entry` — or, since ten of
   twelve actions take it, invert the grammar: `artifact id (all actions except find/create;
   state_at accepts it as an alias of artifact_id)`.
2. Add `#[serde(alias = "id")]` on `state_at::Args::artifact_id`, label `artifact_id` as the
   legacy spelling, and consider dropping it from the schema after a usage window.
3. Describe and label `include_observations` (`get:`), `include_archived`/`limit`/`offset`
   (`find:`).
4. Gates, beside the existing two: (a) every property on every registered tool has a
   non-empty `description` — 12 fail today across 7 tools, listed in the 2026-09-02 review;
   (b) for `artifact` and `librarian`, every property's description starts with an action
   label (or an explicit `all:`), and the union of labels covers every action in the enum
   (`delete` fails today).

Net bytes: +~150 on a surface with 1 char of headroom; fund from the `patch` trim filed as a
sibling. The point is not bytes — it is that a reader of `delete`'s row currently learns nothing.

## Tests added

None yet. Owed: gates 4(a) and 4(b) above.

## Workarounds

None needed; the actions work. The cost is discovery, and the `param_probe` hints on failure
name the field.

## Resume

Edit `src/librarian/tools/artifact.rs:79/90/169` and `src/librarian/tools/state_at.rs:149`; write gates 4(a)/(b) in `src/server.rs`
tests next to `all_tools_have_valid_schemas`; `cargo test --lib artifact`.

## References

- `docs/trackers/prompt-surface-compaction-session-log.md`, 2026-09-02 review section § 2.
- `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` § *Revisit-when* — the
  `schemars` derivation that would make label completeness structural; this file is **not**
  an instance of that class (required-but-unadvertised) and does not move its count.
- `docs/trackers/issue-clusters.md` `IC-11`.

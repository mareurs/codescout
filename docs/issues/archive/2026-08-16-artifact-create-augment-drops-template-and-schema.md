---
kind: bug
status: fixed
tags:
- librarian
- artifact-create
- silent-drop
- doc-vs-code-drift
- cluster/accepted-parameter-silently-dropped
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-07-13-artifact-create-drops-topic.md
- docs/issues/archive/2026-07-20-artifact-update-toplevel-status-param-silently-dropped.md
- docs/trackers/open-issue-work-queue.md
severity: medium
---

# BUG: artifact(create)'s `augment` silently discards render_template, params_schema and entry_collection — and tracker_design tells you to pass them

## Summary

`artifact(action="create", augment={…})` accepts only `prompt` and `params`. The other five
augmentation fields — `render_template`, `params_schema`, `entry_collection`, `append_mode`,
`history_cap` — are **hardcoded to `None`/`false`** at the insert, and unknown keys inside `augment`
are silently ignored by serde. The call returns success with an id, so a tracker created "atomically
with its augmentation" is in fact only half-configured, and nothing says so.

This is a recurrence of a **fixed** defect class in the same file:
`docs/issues/archive/2026-07-13-artifact-create-drops-topic.md` — *"not a recognized create param,
hardcoded to null"*. `topic` was fixed; the `augment` sub-object still has the hole, five fields wide.

## Symptom (Effect)

No error. Observed 2026-08-16 while creating `docs/trackers/open-issue-work-queue.md`:

```
artifact(action="create", kind="tracker", …,
         augment={prompt: …, params: {…},
                  params_schema: {…},        ← accepted, discarded
                  render_template: "…"})     ← accepted, discarded
→ { "id": "9a892c2a5976e296", "abs_path": "docs/trackers/open-issue-work-queue.md" }
```

Reading it back:

```
artifact(action="get", id="9a892c2a5976e296")
→ augmentation.render_template : null
  augmentation.params_schema   : null
  augmentation.entry_collection: null
```

Recovering required a second call — `artifact_augment(id=…, merge=true, render_template=…,
params_schema=…, entry_collection=…)` — which succeeded, proving the fields are supported and only
the create path cannot set them.

## Reproduction

```
1. artifact(action="create", kind="tracker", title="X", rel_path="docs/trackers/x.md",
            body="…", augment={"prompt": "p", "render_template": "T", "entry_collection": "rows"})
2. artifact(action="get", id=<returned id>)
→ render_template and entry_collection are null; the call reported success.
```

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP stdio. Reproduced against the running server,
not only read from source.

## Root cause

**Two independent causes, both in `src/librarian/tools/create.rs`.**

**1. The deserializer has no field for them.** `AugmentSpec` (`src/librarian/tools/create.rs:39-42`)
declares exactly two:

```rust
#[derive(Debug, Deserialize)]
pub struct AugmentSpec {
    pub prompt: String,
    pub params: Option<Value>,
}
```

`#[derive(Deserialize)]` without `#[serde(deny_unknown_fields)]` **ignores unknown keys**, so
`render_template` inside `augment` is discarded during parsing — before any code could reject it.

**2. The insert hardcodes the rest.** Even with a value in hand there is nowhere to put it: the
`AugmentationRow` built at `src/librarian/tools/create.rs:249-265` pins five fields as literals:

```rust
render_template: None,
params_schema: None,
append_mode: false,
history_cap: None,
entry_collection: None,
```

`measured 2026-08-16` — reproduction above run against the live server, and both sites read at the
bytes.

**The asymmetry is the sharp part.** The sibling write surface **rejects** exactly this mistake:
`artifact(action="update", patch={…})` returns a `RecoverableError` naming the valid fields for an
unknown key — confirmed by real corpus rows (`unknown field 'body_prepend_section', expected one of
'status', 'title', …`). So the same user error is a loud, self-correcting failure on `update` and a
silent data loss on `create`.

## Evidence

### The tool's own guidance instructs the discarded call

`librarian(action="tracker_design")` — which `CLAUDE.md` and `get_guide("librarian")` both tell you
to call *before* creating a tracker — closes with:

> ## Final step
>
> Call `artifact_create` with `kind=tracker`, `status=active`, and `augment={prompt,params}`:
> - `prompt`: the augmentation prompt you wrote in Step 2
> - `params`: the initial params shape from Step 3
> - **`params_schema`: optional, per Step 4**
> - **`render_template`: optional, per Step 5**
> - `body`: from Step 6's skeleton

The list mixes artifact-level fields (`path`, `title`, `topic`, `body`) with augmentation fields, and
names `params_schema` and `render_template` among them. A reader who follows Step 4 and Step 5 —
which the same document spends two sections teaching — will pass both to `create`. They are
discarded. The `augment={prompt,params}` earlier in the sentence is the only contrary signal and
reads as shorthand.

This is what actually happened: the guidance was followed exactly and both fields were lost.

### Blast radius is every augmented tracker created since

Any tracker created through `artifact(create, augment=…)` has `render_template: null` and
`params_schema: null` unless someone noticed and made a second call. A missing `render_template`
means the tracker contributes **no `[LIVE]` block** to `librarian(action="context")` — the tracker
silently stops carrying live state into the surface that exists to deliver it. A missing
`entry_collection` means `entry_filter` queries fail with *"entry_filter set but this artifact is not
augmented"* — which appears in the live usage corpus (2 hits) and reads as a different problem
entirely.

## Hypotheses tried

1. **Hypothesis:** the fields were rejected and the call failed silently upstream.
   **Test:** re-read the returned envelope, then set the same fields via `artifact_augment(merge=true)`.
   **Verdict:** **rejected.** The create returned a valid id, and the follow-up augment set all three
   fields successfully — the storage layer supports them; only the create path cannot reach it.
   **Evidence:** § Symptom.

2. **Hypothesis:** the tool schema is the misleading surface.
   **Test:** read `artifact`'s input schema for `augment`.
   **Verdict:** **rejected** — the tool schema is honest: *"Pass prompt + optional params"*, with only
   those two properties. The misleading surface is `tracker_design`'s Final step.
   **Evidence:** § Evidence, first subsection.

## Fix

**Implemented 2026-08-16 — `0ca6891b` (`experiments`), with a follow-up correction in `7c31f87c`.**
No pending-master-SHA line: the promotion path is fast-forward
(`git rev-list --left-right --count master...experiments` → `0 750`), so these SHAs already *are*
the master SHAs once promoted.

All three parts landed, since fixes 1 and 2 are complementary rather than alternatives.

**1. Reject unknown keys.** `AugmentSpec` now carries `#[serde(deny_unknown_fields)]`
(`src/librarian/tools/create.rs`). A typo inside `augment` fails loudly naming the offending key,
matching `update`'s `patch`. This is worth having even with fix 2 in place, because fix 2 only helps
fields that exist — `render_tempalte` still needs to error.

**2. Accept the full shape.** `AugmentSpec` widened from two fields to all seven caller-controlled
ones (`prompt`, `params`, `render_template`, `params_schema`, `entry_collection`, `append_mode`,
`history_cap`), each threaded into the `AugmentationRow` at the insert instead of being pinned to a
literal. `refreshed_at_commit` stays server-computed and is deliberately **not** caller-supplied.
"Created atomically with its augmentation" is now true rather than partly true, and the mandatory
second call is gone.

**3. The advertised schema now matches**, in both directions. `artifact`'s `augment` property
(`src/librarian/tools/artifact.rs`) documents all seven fields with per-field descriptions and sets
`additionalProperties: false`, so the schema states the rejection rather than leaving callers to
discover it.

**4. `tracker_design`'s Final step is corrected** — the misleading surface this bug's Evidence
section identified. It now prescribes the two-call shape explicitly, states that `prompt` and
`params` were the only fields `create` accepted, warns that `merge=false` resets all seven, and tells
the caller to read the artifact back. Done in the same pass as
`docs/issues/archive/2026-08-16-tracker-design-guidance-always-overflows.md`, as that bug predicted.

**One self-inflicted follow-up.** That Final-step rewrite described the *old* `create` contract, and
part 2 above then made it false — both shipped in `0ca6891b` contradicting each other. Corrected in
`7c31f87c`, caught by reading the live tool output after the rebuild rather than by re-reading the
diff.
## Tests added

In `src/librarian/tools/create.rs`:

| Test | Mutation it catches |
|---|---|
| `create_augment_accepts_the_full_augmentation_shape` | re-pinning any of the five fields to a literal at the insert |
| `create_augment_rejects_an_unknown_field` | dropping `deny_unknown_fields`, restoring silent discard |

The first asserts the **observable** contract — create, then read the row back and check each field
persisted — rather than asserting on `AugmentSpec`'s shape, which would pass while the row still
stored `None`. That distinction is the whole bug: the struct and the stored row disagreed.

Gate: **3842 passed, 0 failed**, `cargo clippy --all-targets -- -D warnings` clean.
## Workarounds

- **Always follow `artifact(create, augment=…)` with
  `artifact_augment(id=…, merge=true, render_template=…, params_schema=…, entry_collection=…)`.**
  `merge=true` matters: `merge=false` overwrites all seven fields, resetting `prompt`/`params` you
  just set.
- After creating any tracker, read it back and confirm the three fields are non-null before assuming
  the tracker renders or filters.

## Resume

N/A — fixed, verified live, archived.

**Live verification, 2026-08-16** (after `cargo rb` + `/mcp`), both halves:

- `artifact(create, augment={… "render_tempalte": "oops"})` → **rejected**, naming all seven valid
  fields, and no artifact created.
- `artifact(create, augment={render_template, params_schema, entry_collection, history_cap})` →
  **all four persisted in one call**, with `append_mode` correctly defaulting to false. The temp
  artifact was deleted through the catalog afterwards.

**One follow-up left open deliberately, not blocking:** trackers created *before* this fix may carry
a null `render_template` from the old behaviour. This needs judgment rather than a blanket repair —
`reflective` trackers legitimately have none by design (`tracker_design` Step 5). Sweep with
`artifact(find, kind="tracker", augmented=true)` and check each.
`docs/trackers/open-issue-work-queue.md` is one known case: it was created *by* this bug and repaired
by hand.
## References

- `docs/issues/archive/2026-07-13-artifact-create-drops-topic.md` — same file, same class, fixed.
- `docs/issues/archive/2026-08-16-tracker-design-guidance-always-overflows.md` — the other defect in the
  same `SYSTEM_PROMPT`.
- `docs/trackers/open-issue-work-queue.md` — BL-18.

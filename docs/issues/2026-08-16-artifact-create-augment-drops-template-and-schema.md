---
status: open
opened: 2026-08-16
closed:
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-07-13-artifact-create-drops-topic.md
  - docs/issues/archive/2026-07-20-artifact-update-toplevel-status-param-silently-dropped.md
  - docs/trackers/open-issue-work-queue.md
tags:
  - librarian
  - artifact-create
  - silent-drop
  - doc-vs-code-drift
kind: bug
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

**1. Reject rather than discard (smallest, highest value).** Add
`#[serde(deny_unknown_fields)]` to `AugmentSpec` (`src/librarian/tools/create.rs:39`). An unknown key
then returns a `RecoverableError` naming the valid fields, matching `update`'s behaviour. This alone
converts silent data loss into a self-correcting error, and the hint can name `artifact_augment` as
the place those fields belong.

**2. Or accept them (better ergonomics, larger change).** Widen `AugmentSpec` to the full seven
fields and thread them into the `AugmentationRow` at `create.rs:249-265`. This makes "created
atomically with its augmentation" true as advertised and removes the mandatory second call. Prefer
this if trackers are expected to be create-and-go.

**3. Either way, fix `tracker_design`'s Final step** (`src/librarian/tools/tracker_design.rs`,
`SYSTEM_PROMPT`) so it does not list fields the named call cannot accept — either by moving
`params_schema` / `render_template` into an explicit follow-up `artifact_augment` step, or by
updating it once fix 2 lands.

**Note the interaction:** fix 3 edits `SYSTEM_PROMPT`, which is the payload measured at 4.1× the
inline budget in `docs/issues/2026-08-16-tracker-design-guidance-always-overflows.md`. Doing both in
one pass is cheaper than twice.

## Tests added

`N/A — not yet fixed.` A regression test should assert the *observable* contract, not the struct:
create with `augment` carrying `render_template`, then `artifact(get)` and assert it is either
present (fix 2) or that the create returned a `RecoverableError` (fix 1). Asserting on `AugmentSpec`'s
fields would pass while the row still stored `None`.

## Workarounds

- **Always follow `artifact(create, augment=…)` with
  `artifact_augment(id=…, merge=true, render_template=…, params_schema=…, entry_collection=…)`.**
  `merge=true` matters: `merge=false` overwrites all seven fields, resetting `prompt`/`params` you
  just set.
- After creating any tracker, read it back and confirm the three fields are non-null before assuming
  the tracker renders or filters.

## Resume

Decide fix 1 (reject) or fix 2 (accept) — they are not exclusive, and fix 1 is worth doing even if
fix 2 lands later, since it also catches genuine typos. Then fix 3 in the same pass as
`docs/issues/2026-08-16-tracker-design-guidance-always-overflows.md`, since both edit `SYSTEM_PROMPT`.

Before fixing, sweep for already-damaged trackers — every augmented artifact with a null
`render_template` is a candidate, though some legitimately have none (`reflective` trackers omit it
by design, per `tracker_design` Step 5). `artifact(find, kind="tracker", augmented=true)` then check
each.

## References

- `docs/issues/archive/2026-07-13-artifact-create-drops-topic.md` — same file, same class, fixed.
- `docs/issues/2026-08-16-tracker-design-guidance-always-overflows.md` — the other defect in the
  same `SYSTEM_PROMPT`.
- `docs/trackers/open-issue-work-queue.md` — BL-18.

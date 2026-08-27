---
id: '48b75c1ccd4f3abb'
kind: bug
status: open
title: 'BUG: 17 librarian entry points answer a required-param miss with a bare serde field name — and on artifact(update) that gate pre-empts an auto-correction written for exactly that call'
tags:
- errors
- librarian
- params
- progressive-disclosure
- policy
opened: 2026-08-27
owner: marius
related:
- docs/adrs/2026-08-27-negative-results-name-their-scope.md
severity: medium
---

## Summary

Seventeen librarian tool entry points deserialize their params with
`serde_json::from_value(args)?`, so a missing required field surfaces as serde's raw
message — `missing field \`patch\`` — which names the field but not the action that wanted
it, offers no corrected call, and does not attempt repair. codescout's *core* tools do
not behave this way: `src/tools/core/params.rs` has a hint-bearing, alias-aware helper
family, and the usage DB shows it working. The two layers diverged and the librarian half
was never brought across.

On `artifact(action="update")` this is worse than thin: the serde gate fires **before**
`lift_top_level_param!`, an auto-correction mechanism written across two bug fixes
specifically to repair the call shape the gate rejects. The repair exists, reports itself
via `corrections`, and is unreachable by the most natural spelling of the call.

## Symptom (Effect)

Verbatim, this session, against `b10725ed`:

```
artifact(action="update", id="0000000000000000", status="fixed")
→ missing field `patch`
```

The message names a field. It does not name the action, does not say `patch` is an
object, does not show a corrected call, and does not mention that `status` — the param
actually supplied — is one of seven the tool will happily lift into `patch` on its own.

Other bare instances in the same family, from the usage DB:

```
artifact       | missing field `entry_collection`
artifact       | missing field `id_prefix`
artifact_event | external_signal.source_id required
artifact_event | external_signal.summary required
```

Contrast a core tool, same failure class, same repo:

```
edit_markdown | missing 'heading' parameter — hint: Name the section to edit, e.g.
                heading="## Section". For multiple edits use edits=[{heading, action, content}].
edit_code     | missing 'symbol' parameter — hint: Name the symbol, e.g.
                symbol="MyStruct/my_method" for a method or symbol="my_fn" for a free
                function. `name_path` is accepted as an alias (that is symbols()' name
                for the same address).
```

## Reproduction

Three calls, `b10725ed` on `experiments`, live MCP. The bogus id is deliberate: params are
deserialized at `src/librarian/tools/update.rs:341` and the id is resolved at
`src/librarian/tools/update.rs:364`, so *which* error comes back identifies *which gate
fired* — with zero mutation risk.

```
1. artifact(action="update", id="0000000000000000", status="fixed")
   → missing field `patch`                  # died at the serde gate

2. artifact(action="update", id="0000000000000000", patch={"status": "fixed"})
   → unknown id `0000000000000000`          # reached id resolution

3. artifact(action="update", id="0000000000000000", patch={}, status="fixed")
   → unknown id `0000000000000000`          # ALSO reached id resolution
```

Call 3 is the load-bearing one. An **empty** `patch` object is enough to pass the gate,
after which `lift_top_level_param!` picks the top-level `status` up and applies it. So the
lift handles this call correctly in every respect except that you must first supply a
placeholder that carries no information, and nothing tells you so.

## Environment

- codescout `b10725ed`, branch `experiments`, Linux, stdio MCP transport.
- Project `codescout` at `/home/marius/work/claude/codescout`.
- Census taken from `.codescout/usage.db` on this project, 2026-08-27.

## Root cause

**Two param-validation layers, split by module, with different contracts.**

- `src/tools/core/params.rs:79-165` — `require_param`, `require_param_or`,
  `require_str_param`, `require_str_param_or`, `require_str_param_or_hint`. All return
  `RecoverableError::with_hint`; all consult a per-name hint registry
  (`missing_param_hint`); the `_or` variants accept aliases so a reasonable-but-wrong
  spelling succeeds instead of failing. This is the layer `src/tools/` uses.
- `serde_json::from_value(args)?` — 17 call sites under `src/librarian/tools/`, one per
  entry point (`append_entry:42`, `augment:274`, `context:581`, `create:233`,
  `event_create:273`, `find:556`, `get:90`, `graph:21`, `link:15`, `refresh:27`,
  `refresh_stale:20`, `reindex:81`, `state_at:156`, `timeline:26`, `update:341`,
  `update_entry:52`, `workspace_state_at:89`). The bare `?` propagates serde's
  `Display`, which knows the field name and nothing else — not the tool, not the action,
  not the schema prose sitting next to it.

*Measured 2026-08-27:* `sqlite3 .codescout/usage.db "SELECT tool_name, error_msg, COUNT(*)
FROM tool_calls WHERE err_family='missing_required_param' GROUP BY 1,2"` — the split falls
exactly on that module line. Every hint-bearing message comes from a `src/tools/` tool;
every bare one comes from a librarian tool.

**The `artifact(update)` sub-case is a reachability bug, not just a wording one.**
`src/librarian/tools/update.rs:49-51` declares `patch: UpdatePatch` as the sole field on
`Args` **without** `#[serde(default)]` — every sibling has one, and `UpdatePatch` itself
derives `Default` (`update.rs:8`). So the gate at `update.rs:341` rejects the call, and
control never reaches `update.rs:353-360`, where seven `lift_top_level_param!` invocations
would have promoted the top-level param into `patch` and appended a note to `corrections`
(emitted at `update.rs:641-643`).

That lift is not incidental machinery. `update.rs:298-302` records why it exists: a
top-level param used to be **silently dropped while the call still returned `updated:
true`** — a silent partial success on a write, fixed for `status` in 2026-07-20 and for
`extra`/`owners`/`tags`/`topic`/`time_scope` in 2026-08-14. The repair for that defect
cannot be reached by the call shape that causes it.

**And the good pattern is already in the same function.** `update.rs:334-339` hand-writes
a `RecoverableError::with_hint` for `patch.rel_path`, naming the action, naming the owner
of the field, and routing to `artifact(action="move", …)`. It sits **six lines above** the
bare `?`. Nothing about the librarian module prevents good errors; the deserialization
boundary was simply never treated as an error surface.

## Evidence

### Usage DB census — `missing_required_param` is the 8th-largest error family

```
il3_pipe_to_trimmer        429
il3_shell_on_source        225
il1_read_overlaps_symbol   182
librarian_managed_artifact 142
il2_structural_edit        111
edit_stale_match            93
worktree_activate_required  75
missing_required_param      68   ← this bug's family
```

Of the 68, roughly 11 come from the bare-serde layer (`artifact` ×10, `artifact_event`
×2); the rest carry hints. `artifact | missing field \`patch\`` alone is **8** — the single
largest bare offender, and the second-largest individual message in the whole family.

### The hint layer is visibly still maturing

Two generations of the same `edit_code` message coexist in the DB:

```
edit_code | missing 'symbol' parameter — hint: Add the required 'symbol' parameter
            to the tool call.                                                        ×3
edit_code | missing 'symbol' parameter — hint: Name the symbol, e.g.
            symbol="MyStruct/my_method" … `name_path` is accepted as an alias.       ×7
```

So an improvement effort is already underway on `src/tools/`. The librarian tools were
never in its scope.

### The schema documents the defect instead of fixing it

`src/librarian/tools/artifact.rs:136`, the `patch` description, reads in part:

> REQUIRED for action='update' — an update with no `patch` fails with the bare serde
> message `missing field 'patch'`, which names the field but not the action that wanted it.

The behaviour is understood well enough to be written down for callers. It just lands in a
2,000-character schema description that the failing call did not read and is not shown.

## Hypotheses tried

1. **Hypothesis** — the message is bare because `patch` is genuinely required and no
   repair is possible.
   **Test** — call 3 in *Reproduction*: `patch={}` plus top-level `status`.
   **Verdict** — **rejected**. It returns `unknown id`, i.e. it passed deserialization,
   and the lift then applies `status` correctly. An empty object carries no information
   the caller did not already supply, so the requirement is syntactic, not semantic.
   **Evidence** — *Reproduction*, call 3.

2. **Hypothesis** — this is a librarian-wide design choice (serde derive everywhere,
   errors by convention terse).
   **Test** — read `update.rs:334-339`.
   **Verdict** — **rejected**. A hand-written `RecoverableError::with_hint` sits six lines
   above the bare `?` in the same function. The two coexist by accident, not by policy.

3. **Hypothesis** — it is rare enough not to matter.
   **Test** — usage DB census.
   **Verdict** — **rejected**. 68 in the family, 8th-largest; 8 for `missing field
   \`patch\`` alone.

## Fix

**Not yet implemented — this file records the rule and the scope.** The rule is not
invented here; it is generalized from three mechanisms already shipped in this repo
(`lift_top_level_param!`, the `corrections` output channel, and
`require_str_param_or_hint`).

### The rule

> A required-parameter failure must do **one** of these, never neither:
>
> 1. **Repair it and say so.** When the call is unambiguously recoverable — the value was
>    supplied under a known alias, or at the wrong nesting level, or the field's absence
>    has exactly one sensible reading — apply the repair and report it in `corrections`.
>    Silence is not acceptable even when the repair is right: an unreported repair teaches
>    the caller nothing and makes the next call identical.
> 2. **Refuse with a route.** Name the tool and action that wanted the field, and give a
>    concrete corrected call — not a restatement of the field name.
>
> And a third clause, which is what keeps clause 1 safe:
>
> 3. **Never repair an ambiguous call.** Two readings that disagree must error and say
>    both readings. This is already implemented at `update.rs:307-317`: when a top-level
>    param and its `patch.` twin hold *different* values, the lift refuses; when they
>    agree, it repairs silently, because agreement is not ambiguity.

Clause 2 is the same principle as
`docs/adrs/2026-08-27-negative-results-name-their-scope.md`, one level up — that ADR
governs a tool's *results*, this governs its *errors*. A zero that does not name what it
examined and a rejection that does not name what wanted the field are the same defect: a
frame the caller cannot see, reported as a fact.

### Scope — why this is a big task

- **17 call sites** to convert, listed under *Root cause*.
- **A per-tool policy decision at each one**: which fields are alias-repairable, which are
  nesting-repairable, which genuinely must error. That judgement cannot be mechanized —
  it is why this is not a sed.
- **A shared mechanism to choose.** Three options, in increasing order of work:
  1. Keep serde derive, catch the error, and re-map it through a librarian-side table
     keyed by `(tool, action, field)`. Cheapest; leaves two layers.
  2. Give `patch` (and the other repairable requireds) `#[serde(default)]` so the existing
     lift machinery becomes reachable, then re-map what remains. Fixes the sharpest case
     first and is independently shippable.
  3. Route librarian params through `src/tools/core/params.rs`, collapsing the fork.
     Correct end state; largest diff; needs each `Args` struct unpicked.
- **`corrections` needs to reach the caller reliably.** It exists on `update` and `find`
  today. If clause 1 generalizes, the field should be uniform across librarian responses,
  and its presence should be documented where callers actually look.

### Suggested first slice (independently shippable)

Option 2 restricted to `artifact(action="update")`: add `#[serde(default)]` to
`Args::patch` at `update.rs:51`. `UpdatePatch` already derives `Default`, so an absent
`patch` becomes an empty patch, the seven existing lifts fire, and the call reported in
*Reproduction* call 1 starts succeeding with a `corrections` note instead of failing.

Guard it: an update carrying **neither** `patch` **nor** any liftable top-level param must
still error — otherwise the change converts a loud failure into a silent no-op that
returns `updated: true`, which is precisely the defect `update.rs:298-302` was written to
close. That guard is the whole risk of the slice, and it needs its own test.

SHA: not yet fixed. patch-id: not yet fixed.

## Tests added

None — not yet fixed. When the first slice lands it needs at least:

- `update_without_patch_lifts_top_level_params_and_reports_the_correction`
- `update_with_neither_patch_nor_liftable_params_still_errors` (the guard above; this is
  the regression test for the silent-no-op risk the fix introduces)
- a librarian-wide test asserting no entry point returns a message matching
  `^missing field` — the equivalent of `prompt_surfaces_reference_only_real_tools`, so the
  18th call site cannot be added silently.

## Workarounds

- `artifact(action="update", …)` — always pass `patch`, even empty: `patch={}` plus
  top-level params works today and reports the lift under `corrections`.
- Canonical form remains `patch={"status": …}`; it never needed the workaround.
- For the other bare messages, the field name is accurate — the missing piece is only the
  action context. `artifact.rs`'s schema descriptions do document each field's owning
  action; read the description for the named field.

## Resume

Decide between options 1/2/3 in *Fix § Scope* before writing code — the choice determines
whether this is one commit or a work stream, and options 1 and 3 are mutually exclusive
end states.

If starting: implement the suggested first slice. Add `#[serde(default)]` to
`Args::patch` (`src/librarian/tools/update.rs:51`), then write
`update_with_neither_patch_nor_liftable_params_still_errors` FIRST and watch it fail —
that guard is the only thing standing between this fix and a re-run of the 2026-07-20 /
2026-08-14 silent-partial-success bug. Re-run the three calls in *Reproduction* against
the rebuilt binary; call 1 must change and calls 2 and 3 must not.

Re-take the census afterwards to confirm the family shrinks rather than moves:
`sqlite3 .codescout/usage.db "SELECT tool_name, error_msg, COUNT(*) FROM tool_calls WHERE
err_family='missing_required_param' GROUP BY 1,2 ORDER BY 3 DESC;"`

## References

- `src/librarian/tools/update.rs:49-51` — `Args::patch`, the only field lacking
  `#[serde(default)]`
- `src/librarian/tools/update.rs:298-329` — `lift_top_level_param!` and the history of the
  silent-drop bug it closes
- `src/librarian/tools/update.rs:334-341` — the good error and the bare `?`, six lines apart
- `src/librarian/tools/update.rs:641-643` — the `corrections` output channel
- `src/tools/core/params.rs:79-165` — the hint-bearing helper family the librarian tools
  do not use
- `src/librarian/tools/artifact.rs:136` — the schema description that documents this
  defect rather than fixing it
- `src/usage/db.rs` — `normalize_err_family`, which already classifies
  `missing_required_param`; this is why the census above was one query
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the same principle applied
  to results rather than errors
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — output sizing and agent-guidance patterns


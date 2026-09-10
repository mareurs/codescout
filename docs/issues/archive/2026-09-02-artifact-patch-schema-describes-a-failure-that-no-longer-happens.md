---
kind: bug
status: fixed
tags:
- cluster/doc-contradicted-by-code
closed: null
fix_patch_id: 371bee7c5081481311866cd797d7591abb2c5bc3
fix_sha: dac1068a5e9f9f3265da091dcda1fa20f9da8592
fixed: 2026-09-09
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md
severity: low
---

# BUG: `artifact.patch`'s schema text describes a failure that `60df0d76` removed, on every request

## Summary

The largest single parameter on the tool surface — `artifact.patch`, 1,102 characters —
opens with: *"REQUIRED for action='update' — an update with no `patch` fails with the bare
serde message `missing field 'patch'`, which names the field but not the action that wanted
it."* That was true until 2026-08-27. `60df0d76` gave `Args::patch` `#[serde(default)]`, so
an update with no `patch` and a liftable top-level param now **succeeds** with a
`corrections` note, and one with nothing to change is refused with a message that names
the action. The sentence now documents a failure a reader will never observe, says
*REQUIRED* of a field the code defaults, and is re-read at cache-read rates on every
request of every session.

## Symptom (Effect)

Wire (`tools/list`, 2026-09-02), `artifact.properties.patch.description`, first 190 chars:

```
REQUIRED for action='update' — an update with no `patch` fails with the bare serde message
`missing field 'patch'`, which names the field but not the action that wanted it. Fields to change. …
```

Actual behaviour since `60df0d76`, live-verified 2026-08-27 18:48 in the related bug file:

```
artifact(update, id, status="fixed")   → updated: true, corrections: ["lifted top-level `status` into `patch.status` …"]
artifact(update, id, patch={})         → refused: "was given nothing to change"
artifact(update, id, commit_refresh=true) → {updated: true, committed: false}
```

## Reproduction

`git rev-parse HEAD` → `4dc0daa2`. Read the wire text with
`python3 scripts/probe_tool_surface.py --json`, then:

```
git show 60df0d76 --stat        # "fix(librarian): artifact(update) reaches its own repair machinery"
```

and `src/librarian/tools/update.rs:95-96`:

```rust
#[serde(default)]
patch: UpdatePatch,
```

## Environment

Not environment-dependent; a property of the source at `4dc0daa2`.

## Root cause

The description at `src/librarian/tools/artifact.rs:135` was written to warn about the
serde-gate defect filed as
`docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md`. The
fix for that defect (`60df0d76`, 2026-08-27) changed `update.rs` and did not revisit the
schema prose that described the pre-fix behaviour — the prose was true when written, and the
diff that falsified it touched a different file. The fix commit's own message is what
records the new contract; nothing propagated it to the surface agents read.

No gate relates a param description to the behaviour it claims. The related bug file's own
`Fix` section verified four call shapes live and did not list the schema sentence among the
surfaces to update.

Measured 2026-09-02: wire dump; `git show 60df0d76`; `update.rs:95-96`.

## Evidence

### Wire text
`tools_list.json` in the session scratchpad (
`/tmp/claude-1000/-home-marius-work-claude-codescout/2cb44cd3-8673-4604-a8ac-5adea75ca54b/`).

### Fix that falsified it
`60df0d76` — commit body: *"`Args::patch` was the only field on the struct without
`#[serde(default)]` … Defaulting it lets deserialization succeed so the seven lifts can run,
and the call now returns `updated: true` with a `corrections` note instead of failing."*

### The probe that nearly re-scoped work on the dead message
`scripts/probe_tool_surface.py` module docstring, trap 4: `missing field patch` counted 11
in usage.db, all 11 before `60df0d76`, zero after — recorded there because a task was
scoped on the count before the dates were checked. The schema sentence is the same dead
defect, published on a surface no date-bounding reaches.

## Hypotheses tried

1. **Hypothesis:** the sentence is still true for some `update` shape. **Test:** read
   `update.rs:88-100` and the four live shapes in the related bug's Fix. **Verdict:**
   rejected — with `patch` defaulted, serde cannot emit `missing field 'patch'` for this
   struct at all.

## Fix

Fixed in `dac1068a` (patch-id `371bee7c5081481311866cd797d7591abb2c5bc3`) — Task 3 of the
tool-surface collapse, whose subject names this directly: *"retire the stale patch text"*.

The sentence describing the `missing field 'patch'` serde failure is gone. `patch`'s description
now opens *"update: the fields to change. Accepted keys: …"* and states the real contract — that
top-level params are lifted and reported under `corrections`, and that an update changing nothing
is refused.

**Verified 2026-09-09 with a control, so the zero is a measurement rather than a broken grep:**
over `src/librarian/tools/artifact.rs`, `"REQUIRED for action='update'"` → **0**,
`"missing field 'patch'"` → **0**, and the control `"Accepted keys"` → **1**. The commit that
removed the sentence is `dac1068a`, confirmed by `git log -S"missing field 'patch'"` rather than
read off the subject line.
## Tests added

None yet. Owed: a pin that `artifact`'s schema text contains neither `missing field` nor
`REQUIRED for action='update'` — an absence assertion, monotone under removal, so annotate
it as a pin on a retired phrase rather than credit it as coverage of the contract. The
contract itself is already covered by `60df0d76`'s tests on `update.rs`.

## Workarounds

None needed; the behaviour is correct, only the description is stale.

## Resume

Edit `src/librarian/tools/artifact.rs:135`; run `cargo test --lib tool_surface` and ratchet
`TOOL_SURFACE_CHAR_BUDGET` down by the chars freed.

## References

- `60df0d76` / patch-id `0ed42a3f21585845d1993da053bc19441d816693` (from the related bug file).
- `docs/trackers/prompt-surface-compaction-session-log.md`, 2026-09-02 review section.
- `docs/trackers/issue-clusters.md` `IC-11`.

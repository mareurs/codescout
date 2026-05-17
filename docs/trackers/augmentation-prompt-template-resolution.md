# Augmentation Prompt Template Resolution

**Status:** Scoping — three options on the table, no decision  
**Origin:** `docs/trackers/i1-session-friction.md` F-9 (closed `mitigated`
2026-05-17 second session — Option 1 workaround shipped for L1)  
**Decision-by:** next `archetype_goal()` prompt iteration that touches
more than one active tracker.

## Problem

`artifact_augment(id, prompt=...)` writes the prompt string verbatim into
the `artifact_augmentation` SQL row at creation time. There is no
template-resolution at refresh time. When `archetype_goal()` in
`src/librarian/tools/tracker_design.rs` changes, the new prompt only
reaches **future** trackers created via
`librarian(action="tracker_design", intent="goal: ...")` +
`artifact(create, augment=...)`. Existing trackers keep the
prompt-at-creation, even after `cargo build --release` + `/mcp` reload.

**Concrete change scenario this design must absorb:** editing
`archetype_goal().prompt_template` should propagate to existing
goal-trackers without requiring N manual
`artifact_augment(merge=false, prompt=<fresh>)` calls — one per tracker.

## Options

### Option 1 — Manual re-augment per tracker (workaround, shipped)

**Decision:** documented in F-9 as the post-Phase-1 patch; executed for
the L1 dogfood goal-tracker (`d2cd00fc837e53f2`) during the H-8 close
(see W-10 in the I1 friction log for the scouting evidence).

**Cost:** linear in the number of existing trackers per prompt edit.
Tedious, easy to forget, no detection of drift.

**Used as:** the immediate workaround. Not a long-term design.

### Option 2 — Template field on augmentation row

Store the archetype name (e.g. `"goal"`, `"audit_issues"`) on the
augmentation row. At refresh time, if archetype is set, resolve the
prompt from `tracker_design.rs::archetype_<name>()` instead of reading
the stored string.

**Coupling change:**

- New column on `artifact_augmentation`: `archetype TEXT NULLABLE`.
- Refresh path checks `archetype` and dispatches to the corresponding
  Rust function for the live template.
- `artifact_augment(merge=false)` either preserves the existing archetype
  field across calls, or requires callers to repass it (per F-16 sibling
  semantics — the same destructive-on-merge=false issue applies).

**Tradeoffs:**

- **Now easier:** prompt edits ship in source code; take effect on the
  next release without re-augmenting any tracker.
- **Now harder:** the stored prompt becomes a fallback rather than the
  source of truth. The archetype-name registry becomes a coupling point
  (rename costs propagate). Trackers without an archetype (free-form)
  bypass resolution and behave like Option 1.
- **Revisit-when:** a 4th distinct archetype is added (the resolver
  registry starts to need a real dispatch table, not three branches).

### Option 3 — `prompt_version` mismatch trigger

Add a `prompt_version` integer (or content hash) to the augmentation row.
Each archetype's prompt template gets a corresponding version constant
in source. At refresh time, if stored version differs from current,
`artifact_refresh` returns a "stale prompt" warning suggesting a
re-augment.

**Coupling change:**

- New column on `artifact_augmentation`:
  `prompt_version INTEGER NULLABLE` (or `prompt_hash TEXT NULLABLE`).
- Refresh path computes current version/hash, compares to stored, emits
  a warning when mismatched.
- The agent (or user) decides whether to re-augment. Manual but informed.

**Tradeoffs:**

- **Now easier:** stale prompts are flagged automatically. No silent
  drift between source and stored.
- **Now harder:** still requires a re-augment to fix — only the
  detection is automated, not the propagation. The warning rate could
  become noise if prompt edits are frequent.
- **Revisit-when:** the warning rate is high enough that automatic
  propagation is justified — then promote to Option 2.

## Decision criteria

The right option depends on the rate of `archetype_*().prompt_template`
iteration cross-multiplied with the number of active trackers per
archetype:

| Edit cadence | Active trackers per archetype | Recommended |
|---|---|---|
| ~1 edit / quarter | 1–3 | Option 1 (manual) — current state |
| ~1 edit / month | 1–3 | Option 3 (detection, manual fix) |
| ~1 edit / month | 10+ | Option 2 (full propagation) |
| ~1 edit / week | any | Option 2 |

**Today's state:** ~3 prompt edits over the I1 work, 1 active goal-tracker
(L1 dogfood). Option 1 is fine. Option 3 becomes worthwhile once
goal-trackers proliferate across projects.

## Promote-when (graduate this tracker into an ADR + plan)

Promote out of `scoping` when **any** of:

- A second `archetype_*` (e.g. `archetype_audit_issues`) ships and
  exhibits the same per-artifact prompt drift; OR
- The number of active goal-trackers in the workspace exceeds 5; OR
- A user-facing report (dashboard, audit) depends on "is this tracker
  on the latest prompt" — drift becomes externally visible, no longer
  just an LLM-internal concern; OR
- A prompt edit lands that must reach every existing tracker within
  the same session (no time to enumerate and re-augment manually).

## Stale-when

This tracker becomes wrong when **any** of:

- Option 2 or Option 3 ships. At that point: archive this tracker, link
  the ADR / plan that replaced it.
- `artifact_augment` is replaced or its storage model changes (e.g. the
  prompt moves out of the SQL row and into the augmentation prompt's
  rendered text). The drift problem changes shape; this tracker's
  problem statement becomes outdated.
- The rate of `archetype_goal()` edits drops to zero for >6 months
  (i.e. the prompt template stabilizes). At that point Option 1 is
  permanent and Options 2/3 should be archived as `wontfix`.

## Status

Scoping — Option 1 shipped as workaround; Options 2/3 deferred pending
the Promote-when criteria.

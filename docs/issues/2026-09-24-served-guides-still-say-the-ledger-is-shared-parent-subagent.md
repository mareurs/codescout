---
id: '8bb2ec02b621c3f7'
kind: bug
status: fixed
title: 'BUG: four served surfaces still say the guide ledger is shared parent↔subagent — false since the principal ADR whenever the companion stamps subagent calls'
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-24
opened: 2026-09-24
owner: marius
related:
- docs/adrs/2026-09-14-a-subagent-is-a-principal.md
severity: low
---

# BUG: four served surfaces still say the guide ledger is shared parent↔subagent — false since the principal ADR whenever the companion stamps subagent calls

## Summary

`docs/adrs/2026-09-14-a-subagent-is-a-principal.md` gave each stamped subagent its own guide ledger. Four surfaces served to agents as live instructions still state the pre-ADR model — that the ledger is shared, so a topic the parent triggered "will not auto-inject" for a subagent. Under the companion's principal stamp (Claude Code, the default here) that is false: the subagent receives each auto-inject on its own first touch.

## Symptom (Effect)

`project-activation-bootstrap`, served three times to one conversation on 2026-09-24, § *When you dispatch subagents — brief them*:

```
the guide-hint ledger is shared parent↔subagent, so a topic you triggered will not
auto-inject for them
```

while a stamped probe subagent in the same session received `project-activation-bootstrap` and `symbol-navigation` auto-injects of its own, persisted to its own `~/.local/state/codescout/guide_hints/<session>_<agentId>.json`.

## Reproduction

`git grep -n -e 'shared parent' -e 're-fire for subagents' 09f7b2c2 -- src/prompts src/tools/guide.rs` → the four sites below.

## Environment

codescout `experiments` @ `09f7b2c2`; served via `get_guide` / auto-inject.

## Root cause

The ADR changed ledger ownership in `src/server.rs` (`adopt_request_conversation`, per-principal `GuideLedger` files) and did not sweep the prose that described the old ownership. Sites:

- `src/prompts/guides/project-activation-bootstrap.md` § *When you dispatch subagents — brief them*
- `src/prompts/guides/iron-laws-detail.md` — the "State which get_guide topics you've already triggered" bullet
- `src/prompts/guides/workspace-state.md` § *Subagent semantics* — "The same `guide_hints_emitted` set (parent-triggered hints don't re-fire for subagents)"
- `src/tools/guide.rs` — `get_guide`'s repeat-fetch note ("since the ledger is shared parent↔subagent and a subagent's first fetch always lands here"), plus two code comments repeating the premise

measured 2026-09-24: per-principal ledger files observed live; the grep above.

## Evidence

See Symptom. The *advice* in each site (tell the subagent to fetch; never tell it guides are already loaded) stays correct — the subagent's context still holds none of the parent's guides. Only the stated *reason* is stale, and it is still true for an **unstamped** subagent, so each fix states both cases rather than inverting the claim.

## Hypotheses tried

N/A — doc-vs-code drift, read directly.

## Fix

Reword each site to condition the shared-ledger claim on the absence of the companion's per-subagent stamp. `get_guide`'s repeat note keeps every property `repeat_fetch_keeps_body_and_flags_static` pins ("already delivered"; "if it is not already in your context, read it"; never "You already fetched"), since without the stamp a subagent's first fetch still lands on that branch.

**Implemented**: each of the four sites now states the shared-ledger claim conditionally (true only without the companion's per-subagent stamp) and keeps its advice; the two `guide.rs` code comments repeating the premise updated to match. `repeat_fetch_keeps_body_and_flags_static` passes against the new note; gate `GATE_EXIT=0`, which includes `a_p50_session_stays_under_the_committed_emission_byte_ceiling` for the longer bodies.

- **SHA** — `a126bf482597e5f691dab5c4edaaeda63d83a997` (branch `experiments`).
- **patch-id** — `01e28ebfd6aeb2d9f488e404055d7c4655fcff4d`.

## Tests added

N/A for the prose — asserting on wording reds on every rewording (CLAUDE.md § *Testing Discipline*). The one behavioural property at stake, that the repeat note stays neutral about who received the guide, is already pinned by `repeat_fetch_keeps_body_and_flags_static` (`src/tools/guide.rs`), which passes against the new wording.

## Workarounds

None needed; the advice itself was right.

## Resume

Archive via `doc(action="move")`, re-pointing `deep-agent-workflow-observations:DWF-6`'s citation of this file's id in the same commit (the move mints a new id). Deferred only because that ledger file currently carries two peer sessions' uncommitted entries.

## References

- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md`
- `docs/issues/2026-09-24-guide-rearm-request-is-consumed-by-whichever-principal-calls-next.md` — found in the same verification

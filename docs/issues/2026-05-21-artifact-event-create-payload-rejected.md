---
status: fixed
opened: 2026-05-21
closed: 2026-05-21
severity: medium
owner: marius
related: []
tags: [librarian, artifact-event, mcp-schema, payload]
kind: bug
---

# BUG: artifact_event(create) rejects every payload with "payload must be object"

## Summary
`artifact_event(action="create", ...)` rejects all payloads — including a
minimal `{"intent": "seed"}` — with the error `payload must be object`,
making it impossible to append `note`/`intent`/`external_signal` events to
any artifact. Blocks seeding the inaugural `intent` event on the TimeMachine
pivot tracker (`738ba3a92de509d6`) and, more broadly, the entire
event-driven observation workflow that watch-trackers depend on.

## Symptom (Effect)
Every `create` call returns:

```
payload must be object
```

Reproduced with three payloads of decreasing complexity (full intent object,
section-ref-sanitized object, and a one-key `{"intent": "seed"}`) — all
rejected identically. `artifact_event(action="list", artifact_id=...)`
on the same artifact succeeds (returns `[]`), so the tool is reachable and
the failure is specific to the `create` path's payload handling.

## Reproduction
```
git rev-parse HEAD   # 26618957ba1a9341bd61b421fc99b526b3c577aa
```
1. `artifact_event(action="create", artifact_id="738ba3a92de509d6",
   kind="intent", payload={"intent": "seed"})`
2. Observe: `payload must be object`.
3. Contrast: `artifact(action="update", id="738ba3a92de509d6",
   patch={"topic": "x"})` — succeeds. `patch` declares `"type": "object"`
   in its JSONSchema; `payload` does not.

## Environment
- Project: code-explorer, branch `experiments`, HEAD `26618957`.
- MCP transport: codescout MCP server (release binary via `~/.cargo/bin/codescout` symlink).
- Client: Claude Code (FleetView harness).

## Root cause
*Leading hypothesis (confirmed by discriminating test, not yet by wire capture).*

The `payload` property in `artifact_event`'s input JSONSchema has **no
`"type"` field** — only `"description": "create: event payload (any JSON)"`.
Object-typed parameters that DO declare `"type": "object"` (e.g.
`artifact.patch`) round-trip an object correctly through the same client.
The asymmetry indicates the MCP client serializes the typeless `payload`
value to a JSON **string** before transport; the server then receives a
string and its `payload must be object` guard rejects it.

Discriminating test (this session): same artifact, same client —
`patch={"topic": ...}` (typed object) succeeded; `payload={...}` (typeless)
failed. This isolates the defect to the schema's missing type annotation on
`payload` rather than to the server's write logic or the artifact's state.

Fix candidates (need source confirmation in the artifact_event tool def):
1. Add `"type": "object"` to the `payload` property schema so the client
   transports it as an object. (Most likely correct — mirrors `patch`.)
2. If "any JSON" (non-object payloads) is genuinely intended, have the
   server accept a JSON-string and parse it, instead of hard-rejecting
   non-objects.

## Evidence
### E1 — three rejected create calls
All three returned `payload must be object`:
- full intent object (intent/decision_question/reevaluate_when/spec_ref/signals_fired/status_at_seed)
- same object with "§12" replaced by "section 12" (ruled out a unicode-in-payload theory)
- minimal `{"intent": "seed"}`

### E2 — list works, create does not
`artifact_event(action="list", artifact_id="738ba3a92de509d6")` → `[]`.
Tool reachable; artifact has zero events; failure is create-path-specific.

### E3 — typed-object control succeeds
`artifact(action="update", id="738ba3a92de509d6", patch={"topic":
"timemachine-pivot-watch"})` → `{"updated": true}`. `patch` schema has
`"type": "object"`; `payload` schema does not.

## Hypotheses tried
1. **Hypothesis:** Payload content (unicode "§", nested keys) trips a
   validator. **Test:** sanitized "§12"→"section 12", then reduced to
   `{"intent":"seed"}`. **Verdict:** rejected — minimal payload still fails.
   **Evidence:** E1.
2. **Hypothesis:** Tool/artifact unreachable or wrong id. **Test:** `list`
   on same id. **Verdict:** rejected — list returns `[]` cleanly.
   **Evidence:** E2.
3. **Hypothesis:** Missing `"type": "object"` on the `payload` schema causes
   client to stringify it. **Test:** compare against `patch` (typed object)
   on the same artifact/client. **Verdict:** confirmed (behaviorally) — typed
   object succeeds, typeless object fails. **Evidence:** E3.

## Fix

Implemented. Added `"type": "object"` to the `payload` property in the
hand-written MCP input schema at `src/librarian/tools/artifact_event.rs:36`
(mirroring how `artifact.patch` declares its object type). Updated the property
description from "any JSON" to "a JSON object" to match the server's
`.as_object()` contract.

Verified live: after `cargo build --release` + `/mcp` reconnect, an object
payload is accepted (the "payload must be object" rejection is gone), and the
kind-specific `intent.hypothesis required` validation then fires correctly — so
the full create path works. Seeded the inaugural intent event
`01KS5M9N2BEN13J64RS8MSHAJZ` on the TimeMachine pivot tracker as the
confirming end-to-end test.
## Tests added

`payload_schema_declares_object_type` in `src/librarian/tools/artifact_event.rs`
(tests module) — asserts `input_schema()["properties"]["payload"]["type"] ==
"object"`. This is the schema-level regression: it fails if the `type`
annotation is ever dropped again. The full create round-trip for `intent` events
is already covered by existing tests in `event_create.rs` (e.g.
`payload: {"hypothesis": "X causes Y"}`).
## Workarounds
None known from the client side — the rejection is deterministic for every
object payload. Watch-trackers that depend on `intent`/`note` events
(e.g. the TimeMachine pivot tracker) cannot be seeded until the schema is
fixed. Interim: record observations as prose in the tracker body via
`edit_markdown` instead of as events (loses the event-graph queryability the
tracker design intended).

## Resume

N/A — fixed and verified live this session.
## References
- Tracker blocked by this bug: `docs/superpowers/trackers/timemachine-pivot-to-codescout.md` (id `738ba3a92de509d6`).
- Spec context: `docs/superpowers/specs/2026-04-28-librarian-timeline-design.md` §12.

---
status: open
opened: 2026-09-02
closed:
severity: low
owner: marius
related:
  - docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md
tags:
  - cluster/doc-contradicted-by-code
kind: bug
---

# BUG: `index`'s description enumerates three actions; the enum, the dispatcher and the callers have four

## Summary

The `index` tool's description reads *"Actions: `build` (…), `status` (…), `cancel` (…)"*.
Its `action` enum is `["build", "status", "cancel", "verify"]`, `verify` dispatches to
`IndexVerify`, and it was called 16 times in the last 30 days. The one action that answers
*"did the index actually cover everything?"* is discoverable only by reading the enum, and
the description that claims to list the actions leaves it out.

## Symptom (Effect)

Wire (`tools/list`, 2026-09-02):

```
description: Semantic index operations. Actions: `build` (build/update the project's semantic
index; pass `scope='lib:<name>'` to index a registered library), `status` (show index stats),
`cancel` (abort an in-flight reindex — no-op if nothing is running).
properties.action.enum: ["build", "status", "cancel", "verify"]
```

usage.db, 30 days to 2026-09-02:

```
SELECT count(*) FROM tool_calls WHERE tool_name='index' AND input_json LIKE '%verify%';  → 16
```

## Reproduction

> **Corrected 2026-09-02.** The original text read: *"`python3 scripts/probe_tool_surface.py
> --json`, find `index`; compare `description` with `inputSchema.properties.action.enum`."*
> That command cannot perform the comparison it prescribes —
> `scripts/probe_tool_surface.py:135-137` emits `"desc": len(t.get("description",""))` and
> `"props": {k: len(dj(v))}`, i.e. character counts with the text discarded. Confirmed by this
> file's author, who reports the dump actually used was an unnamed scratchpad script. The
> working form is below; it reuses that script's `fetch_tools` transport, so it reads the same
> wire the budget probe does rather than introducing a second handshake.

`git rev-parse --short HEAD` → `09c68634`.

```
python3 - <<'PY'
import importlib.util
spec = importlib.util.spec_from_file_location("pts", "scripts/probe_tool_surface.py")
pts = importlib.util.module_from_spec(spec); spec.loader.exec_module(pts)
for t in pts.fetch_tools("target/debug/codescout"):
    enum = (t.get("inputSchema", {}).get("properties", {}).get("action", {}) or {}).get("enum", [])
    if not enum:
        continue
    missing = [v for v in enum if v not in t["description"]]
    if missing:
        print(t["name"], "missing", missing)
PY
```

Reports, over all 26 tools:

```
artifact       missing ['find', 'get', 'create', 'move', 'delete', 'graft', 'link', 'graph', 'state_at']
edit_markdown  missing ['replace', 'insert_before', 'insert_after', 'remove']
index          missing ['verify']
memory         missing ['refresh_anchors']
```

Two of those four are real. `artifact` and `edit_markdown` describe by theme rather than
enumerating, so neither claims to be an action inventory — they are exactly the false
positives § *Fix* below predicts the gate must exclude, and their appearance here is the
crude check behaving as predicted rather than a defect.

`memory` is a **second real instance**, filed separately as
`docs/issues/2026-09-02-memory-description-omits-the-refresh-anchors-action.md`. It is why
§ *Fix*'s sentence *"`index` fails today, the other three pass"* is false: the enumerating
population is five tools, not four, and two of the five fail. See that file's § *Why this
went unfiled* — the omission was in this file's § *Evidence* comparison, so the second
instance was unreachable by reading this record and only a re-run over the whole surface
could find it.
## Environment

Not environment-dependent.

## Root cause

`src/tools/semantic/index.rs:836` (description) names three actions;
`src/tools/semantic/index.rs:848` (enum) and `:879` (`"verify" => IndexVerify.call(…)`) carry
four. `verify` shipped with the fix for
`docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md`;
the enum and dispatcher were updated, the sentence that enumerates them was not.

No test relates a description's enumerated actions to the enum. `all_tools_have_valid_schemas`
checks shape only; `tool_descriptions_stay_under_budget` checks length only.

Measured 2026-09-02: wire dump; `src/tools/semantic/index.rs:836/848/879`; usage.db count.

## Evidence

### Wire
`tools_list.json` in the session scratchpad,
`/tmp/claude-1000/-home-marius-work-claude-codescout/2cb44cd3-8673-4604-a8ac-5adea75ca54b/`.

### Same-surface comparison

Of the tools whose description enumerates actions, `workspace` names 3/3, `library` 2/2,
`edit_code` 4/4, `index` **3/4** (computed over the wire dump, 2026-09-02).

> **Superseded 2026-09-02 — this population is wrong twice over.** It omits `memory`
> (**7/8**, the sibling bug) *and* it is not the whole population: `artifact_event` (2/2),
> `artifact_refresh` (2/2) and `librarian` (**10/10**) also enumerate, and `librarian` is
> exempted by name in § *Fix* above on the false premise that it describes by theme. The
> derived figure, measured by the gate over `server.tools`: **10 tools carry an `action`
> enum; 8 are inventories, 2 (`artifact`, `edit_markdown`) are thematic.** Read § *Fix*.
## Hypotheses tried

1. **Hypothesis:** `verify` is intentionally undocumented (internal/probe action).
   **Test:** 16 calls in 30 days from agent sessions; `IndexVerify` is a public tool struct.
   **Verdict:** rejected.

## Fix

**Implemented 2026-09-02**, together with the sibling `memory` bug and the shared gate.

`src/tools/semantic/index.rs:836` now carries `` `verify` (check coverage against the
filesystem; read-only) ``, worded from `IndexVerify::description()` rather than invented.

Funded **not** from the sibling's `patch` trim but from `index`'s OWN `build` clause, which
restated the `scope` parameter's description almost verbatim: *"build/update the project's
semantic index; pass `scope='lib:<name>'` to index a registered library"* →
*"build/update the index; `scope='lib:<name>'` for a library"*. This mattered: the plan's
`+~60 chars` against a 243-char description would have breached the **300-char per-tool
cap** (`tool_descriptions_stay_under_budget`) at 304. Net actual: +21, to 264. Surface total
56,479 → 56,516 against a 56,519 ratchet, so `TOOL_SURFACE_CHAR_BUDGET` did **not** need
raising.

### Two claims in the plan above were wrong, and both narrow the gate below the defect

1. *"`index` fails today, the other three pass"* — **two** failed. The sibling bug caught
   this one: its own comparison population had omitted `memory`.
2. *"`artifact` and `librarian` describe by theme and would be false positives"* —
   **`librarian` is an inventory, not thematic.** It names all 10 of its actions. Exempting
   it as prescribed would have dropped the largest description on the surface (1,621 chars,
   10 actions) out of the guarded population, and nothing would have reported the hole. The
   actual thematic pair is `artifact` (names 3 of 12) and `edit_markdown` (0 of 5).

The measured population is **10 tools carrying an `action` enum — 8 inventories, 2
thematic** — neither the 4 this file claimed nor the 5 the sibling corrected it to. Three
statements of one population, by two authors, none derived over the whole surface; the two
errors are in *different* directions (a short count, and a mis-assigned member), so the
sibling's correction could not have caught this one.

That is why the gate **derives** the population instead of asserting it: any tool with an
`action` enum and no declared contract fails, so the list cannot silently fall behind the
surface again.
## Tests added

`server::tests::tool_descriptions_name_every_action_they_claim_to_enumerate`, in
`src/server.rs` beside `tool_descriptions_stay_under_budget`.

Contracts are **declared, never inferred**. The `Actions:`-marker sniff this file proposed
is a parser over a namespace with no escape (`CLAUDE.md` § *Parsers Over a Namespace*) and
fails in both directions: a description mentioning "Actions:" in prose is conscripted into a
promise it never made, and one that enumerates without the marker is silently exempted.
Instead each action-enum tool is declared `Inventory` or `Thematic`, and an **undeclared**
tool fails the gate.

Word-boundary, case-sensitive matching, both for live reasons: `artifact_refresh`'s enum is
`["gather", "list_stale"]`, so a substring test for a `list` action is satisfied by
`list_stale`; and `edit_markdown` opens "Edit a Markdown document", where `Edit` is the verb,
not the action. The reference probe in the sibling bug used Python's `in` and inherits the
first hole.

Verified **per member, not once** (`CLAUDE.md` § *Testing Discipline*) — four independent
mutations, each killed:

| mutation | what fired |
|---|---|
| revert `index`'s description | `under_reported`, naming `index` alone |
| revert `memory`'s description | `under_reported`, naming `memory` alone |
| drop a tool from both arms | `undeclared`, naming it and its enum |
| declare a complete tool `Thematic` | `thematic_but_complete` |

The fourth is the one worth keeping: an `Inventory` assertion is monotone under *widening* a
description, so it can never fire on an over-broad exemption. Without the converse check,
`Thematic` would be an escape hatch that only ever grows and silences.
## Workarounds

Read the enum; `index(action="verify")` works.

## Resume

Edit `src/tools/semantic/index.rs:836`; add the gate beside `tool_descriptions_stay_under_budget` in
`src/server.rs`; `cargo test --lib index`.

## References

- `docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md` — where `verify` came from.
- `docs/trackers/prompt-surface-compaction-session-log.md`, 2026-09-02 review section.
- `docs/trackers/issue-clusters.md` `IC-11`.

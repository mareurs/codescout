---
id: '328021e820100805'
kind: bug
status: open
title: 'BUG: is_write omits five mutating actions, so the cross-process write guard never fires for them'
tags:
- cluster/guard-narrower-than-its-name
- librarian
- concurrency
- write-guard
- shared-checkout
- append-entry
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: high
---

# BUG: `is_write` names five mutating actions it does not match, so the cross-process write guard never fires for them

## Summary

`LibrarianAdapter::is_write` is the sole gate on codescout's cross-process write
guard. It enumerates mutating actions by name, and **five mutating actions are
missing from that enumeration** — three on the `artifact` arm and two on the
`librarian` arm. Calls to them take neither the in-process mutex nor the
`.codescout/write.lock` fd lock, on a checkout that routinely has five or more
concurrent sessions.

The sharpest instance: `artifact(action="append_entry")` exists **because**
concurrent sessions race on entry-id allocation, and it is one of the five.

## Symptom (Effect)

No error. No warning. `acquire_write_guard_if_writing` returns `Ok(Ok(None))`
and the call proceeds unserialised:

```rust
// src/server.rs:651
if !self.is_write_call(name, input) {
    return Ok(Ok(None));
}
```

Two concurrent `append_entry` calls against the same ledger from different
sessions are not mutually excluded at any layer.

## Root cause

A four-link chain, every link read this session:

1. `src/server.rs:651` — `acquire_write_guard_if_writing` returns no guard when
   `!is_write_call(...)`.
2. `src/server.rs:537-541` — `is_write_call` delegates to `t.is_write(input)`.
3. `src/librarian/adapter.rs:272-305` — `LibrarianAdapter::is_write`:
   - the `"artifact"` arm is
     `matches!(action, Some("create" | "update" | "move" | "delete" | "link"))`
   - the `"librarian"` arm matches `reindex`, `audit_doc_refs`,
     `legibility_scan`, `link_scan`, then `_ => false`
4. `src/librarian/tools/artifact.rs:227-236` — the dispatcher routes
   `graft` (`:231`), `append_entry` (`:235`) and `update_entry` (`:236`), and
   the schema enum at `:35` lists all three.

**The five unmatched mutating actions:**

| action | mutates | falls through at |
|---|---|---|
| `artifact.append_entry` | writes the file, advances `entry_high_water_<PREFIX>` in committed frontmatter, writes catalog rows | `adapter.rs:275` |
| `artifact.update_entry` | patches an entry in place | `adapter.rs:275` |
| `artifact.graft` | **DELETES** `from_id`'s row after moving its events/links/observations | `adapter.rs:275` |
| `librarian.doctor` with `fix=…, confirm=true` | `std::fs::write` on project files (`doctor.rs:999`) | `adapter.rs:303` (`_ => false`) |
| `librarian.merge_worktree` | `DELETE`s catalog rows | `adapter.rs:303` (`_ => false`) |

**measured 2026-09-02** (subagent sweep over `.codescout/usage.db`, 30-day
window): **557 calls** to the three `artifact` actions. Note the DB records
arguments only under `--debug`, which this machine runs; the count is a lower
bound scoped to MCP calls on this host.

*Links 1–4 read directly and verified this session. The 557 figure is a
subagent's measurement, re-derivable from the query in the References.*

## Evidence

### The design knows about this race on a different axis

`get_guide("tracker-conventions")` justifies server-side id allocation in these
words:

> Hand-allocation races: a peer session in the same checkout can take the id
> between your scan and your write.

The same guide records a **worktree** guard for the same hazard — `append_entry`
refuses id allocation from a worktree session, because the worktree's shadow row
is a different `artifact_id` and both trees would issue the same number.

So the author reasoned about concurrent allocation, guarded the worktree axis,
and left the same-checkout axis — the axis the feature's own rationale names —
unguarded.

### The test cannot fail on an omission

`src/server.rs:5532` — `is_write_call_classifies_plain_writes` asserts
`edit_file`, `create_file`, `edit_code.replace`. Every assertion is a
**membership** assertion over actions that *are* classified. A test of that
shape is monotone under omission: adding a sixth unmatched action reds nothing.

### A prior instance of the same mechanism is archived

`docs/issues/archive/2026-06-01-librarian-adapter-stale-is-write.md` — stale
tool names in this same match arm. The arm has now drifted twice.

## Hypotheses tried

1. **Hypothesis:** the guard is taken somewhere else for these actions.
   **Test:** read `acquire_write_guard_if_writing` and its only gate; grep for
   other `WriteGuard` acquisition sites.
   **Verdict:** rejected — `is_write_call` is the only gate.

## Fix

**Not yet applied.** The correct fix is to make the enumeration exhaustive
*by construction*, not to add five names — the arm has drifted twice already
and a per-action patch ships the same defect a third time.

Options, in preference order:

1. Invert the `artifact` arm: match the **read** actions
   (`find | get | graph | state_at`) and default writes to `true`. A new
   mutating action is then guarded on the day it is added, and a new *read*
   that forgets to opt out is merely over-serialised — the safe direction.
   Same inversion for the `librarian` arm.
2. Move `is_write` next to the dispatcher (`artifact.rs`) so the two lists are
   adjacent and drift is visible in one diff.

Note the `librarian` arm's existing entries are genuinely conditional
(`audit_doc_refs` on `emit_tracker`, `link_scan` on `write == true` — polarity
inverted from `legibility_scan`), so an inversion there must preserve those.

## Tests added

None yet. The regression test must assert the **complement**: for every action
in the `artifact` schema enum, `is_write` returns `true` unless the action is in
an explicit read allowlist. A test that enumerates writes cannot catch the next
omission — see Evidence.

## Workarounds

On a shared checkout, avoid concurrent `append_entry` / `update_entry` /
`graft` against the same ledger from different sessions. The committed
`entry_high_water_<PREFIX>` frontmatter key is the recovery surface if two
sessions do issue the same id.

## Resume

Read `src/librarian/adapter.rs:272-305` and `src/librarian/tools/artifact.rs:35`
(the schema enum) side by side. Invert the `artifact` arm to a read-allowlist,
then write the complement test described in *Tests added* — it should red
before the inversion and green after. Confirm `graft`'s delete path
(`src/librarian/tools/graft.rs`) is covered.

## References

- `src/librarian/adapter.rs:272-305`, `src/server.rs:537-541`, `src/server.rs:651`
- `src/librarian/tools/artifact.rs:35`, `:227-236`
- `src/librarian/tools/doctor.rs:999`
- `docs/issues/archive/2026-06-01-librarian-adapter-stale-is-write.md` — prior drift of this arm
- Count query: `SELECT COUNT(*) FROM tool_calls WHERE tool_name='artifact' AND json_extract(input_json,'$.action') IN ('append_entry','update_entry','graft')` against `.codescout/usage.db`


---
id: 93caba562c06a258
kind: bug
status: fixed
title: 'BUG: is_write omits five mutating actions, so the cross-process write guard never fires for them'
tags:
- cluster/guard-narrower-than-its-name
- librarian
- concurrency
- write-guard
- shared-checkout
- append-entry
closed: 2026-09-02
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

**Fixed on `experiments` at `354ffac4`** (`354ffac4ad11628d02c73ad4d86e191b26af1177`),
patch-id `d4e6237ea3526776bc5b4441abd4677632624c0b`. The SHA is positional and
dies when `experiments` is rebased; the patch-id is a content hash of the diff
and survives rebase and cherry-pick.

Option 1 above, applied at **both grains**:

- `artifact` matches the reads (`find | get | graph | state_at`) and defaults
  every other action to a write, so `graft`, `append_entry` and `update_entry`
  are guarded, as is any action added later.
- The adapter's final `_ => false` arm became `_ => true`. That arm was the same
  defect one level up: every librarian tool is wrapped by a blanket map over
  `lib_all_tools()`, so a tool added later would have arrived unguarded.

**A sixth mutating action, found by reproducing rather than by reading this
file.** `librarian(audit_log)` is not in the table above and writes twice over:
`prune_before_ms` + `confirm=true` reaches `prune_before()` → `DELETE FROM
catalog_audit` (`src/librarian/catalog/audit/mod.rs:411`), and `export=true`
appends to the committed shard (`audit/shard.rs:445`). Two independent
enumerations of one small set — the code's and this report's — and both came up
short. That is the argument for inverting rather than for adding six names.

**Not a blanket default-to-write, and the exception is load-bearing.** `doctor`
and `audit_log` are full-catalog scans in their common form. Guarding them
unconditionally would hold the write lock for the length of a diagnostic and
block every other session's writes behind it — trading this bug for an
availability one. Each keeps a read-default arm keyed on its own
schema-documented repair opt-in, over-approximated: `doctor` guards on `fix`
being present at all, so the dry run that *decides* whether to write is guarded
too.

That exception was not planned — it was surfaced by
`is_write_call_classifies_librarian_surface`, which pins bare `doctor` as a read
and went red on the first attempt. It was right, and it now passes unchanged.
## Tests added

`is_write_classifies_every_action_outside_a_declared_read_set_as_a_write`
(`src/server.rs`), the complement test this section asked for. It derives its
population from each tool's own `action` enum rather than from a list, so:

- a **new action** in the schema is asserted to be a write with no edit to the
  test, and passes only because `is_write` now defaults that way;
- a **new read opt-out** in `is_write` that nobody declares in the test turns it
  red — the safe direction;
- a **stale or misspelled** read-set entry is reported as dead rather than
  silently exempting nothing.

It also pins both polarities of all five conditional arms, which a bare-action
probe structurally cannot reach, and asserts the population is non-empty under
the `librarian` feature so a registration change cannot make it vacuously green.
**It is inert on the lean lane** (`--no-default-features` advertises no
librarian tools) and must not be credited with coverage there.

**Mutation-verified per SITE, not per feature** (`CLAUDE.md` § *Testing
Discipline*):

| mutation | result |
|---|---|
| `artifact` arm reverted to the write enumeration | names all three of `graft`, `append_entry`, `update_entry` |
| `librarian` `_ => true` reverted to `_ => false` | names `reindex` **and** `merge_worktree` |

Neither mutation is caught by the other's assertion, and the second names a
member the pre-existing `is_write_call_classifies_librarian_surface` misses
entirely — that test catches only `reindex`. The two are not redundant.
## Workarounds

On a shared checkout, avoid concurrent `append_entry` / `update_entry` /
`graft` against the same ledger from different sessions. The committed
`entry_high_water_<PREFIX>` frontmatter key is the recovery surface if two
sessions do issue the same id.

## Resume

**Closed and archived.** Gate green on both lanes at the fix commit — lean 3476
passed / 0 failed (29 suites), default 5189 passed / 0 failed (31 suites),
clippy `--workspace --all-targets --features local-embed -D warnings` exit 0.

**The archive was held for about forty minutes and then released, which is worth
recording because the release came from the other side.**
`artifact(action="move")` re-keys on path, and
`docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md:349`
cited this artifact id — in a sentence the fix had already falsified, calling it
*"that predicate's open hole"*. Moving would have converted a stale-but-true
citation into a dangling one, and repairing it meant editing another work
stream's committed design doc where the repair is a judgement about their
reasoning rather than a mechanical repoint. It was routed to that spec's owner
instead, identified from the `Session-Id` trailer on `649db39e` rather than from
proximity to `src/engines/`.

They corrected it at `7795c7c6` and reported **one leg further than was
claimed**: the sentence gave two reasons and *both* expired, not one — the same
commit made the classification per-action, so the tool-level-grain objection
went with the open-hole objection. Their conclusion survives on a reason that
never depended on this code: `is_write` answers *"does this call mutate?"*,
which is a **safety** predicate, while the question they were asking is a
**pedagogy** one, and the two come apart in both directions.

Two things fall out of that exchange and neither is about this bug:

- **An argument from a defect dies when someone fixes the defect.** Their
  retired sentence rested on this hole; the replacement rests on a distinction
  that no fix can remove. Worth preferring the second kind when writing one.
- **A quotation of a retired citation is indistinguishable from a live one.**
  Their correction quotes the retired sentence, which originally reproduced the
  16-hex id — reproducing it would have scheduled exactly the dangling ref the
  hold existed to prevent. They elided it and cited the fix by SHA + patch-id
  instead. That is `IC-6`'s no-escape half, met inside the repair of an unrelated
  defect.

One thing deliberately **not** done: `graft`'s delete path
(`src/librarian/tools/graft.rs`) was not separately covered. It needs no new
coverage for *this* defect — `graft` is guarded by the same inversion as its
siblings and the regression test names it — but no test exercises the delete
itself under concurrency, so that is an untested path rather than a closed one.
Recorded here rather than left implied.
## References

- `src/librarian/adapter.rs:272-305`, `src/server.rs:537-541`, `src/server.rs:651`
- `src/librarian/tools/artifact.rs:35`, `:227-236`
- `src/librarian/tools/doctor.rs:999`
- `docs/issues/archive/2026-06-01-librarian-adapter-stale-is-write.md` — prior drift of this arm
- Count query: `SELECT COUNT(*) FROM tool_calls WHERE tool_name='artifact' AND json_extract(input_json,'$.action') IN ('append_entry','update_entry','graft')` against `.codescout/usage.db`

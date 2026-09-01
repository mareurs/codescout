---
id: '1c5e106ee122f582'
kind: bug
status: open
title: 'BUG: /mcp reconnect applies a CHANGED env var from settings.json but not a REMOVED one — and the change that lands falsely confirms the one that did not'
tags:
- cluster/config-propagation-is-additive
- harness
- mcp-reconnect
- config
- stale-env
- false-confirmation
- not-codescout-source
opened: 2026-08-30
owner: marius
severity: high
unverified: 'Mechanism (merge-over-replace) is INFERRED from behaviour, not read from source — Claude Code is not in this repo. It has since made a correct falsifiable prediction (see § The mechanism made a prediction), which is evidence and not proof. Still untested: whether a full Claude Code restart clears a deleted key.'
---

# BUG: `/mcp` reconnect applies a CHANGED env var from `settings.json` but not a REMOVED one

> **Not a codescout bug.** `settings.json` → MCP `env` injection is Claude Code
> harness behaviour; its source is not in this repo. Filed here under CLAUDE.md's
> "open a bug file for ANY bug noticed during work — including tool
> quirks/misbehaviors", because the cost lands on codescout's retrieval config.

## Summary

One edit to `~/.claude/settings.json` § `env` made two changes:

- **removed** `CODESCOUT_QUERY_PREFIX`
- **changed** `CODESCOUT_BM25_BOOST` from `"3.0"` to `"5.0"`

After `/mcp`, the newly-spawned server has the **new boost** and the **removed
prefix still set**. The update landed; the deletion did not.

## Symptom (Effect)

`printenv` inside the server spawned 11 s after the reconnect:

```
QUERY_PREFIX: Represent this query for searching relevant code:    <- deleted on disk
BM25_BOOST:   5.0                                                  <- new value, applied
```

## Reproduction

1. In `<profile>/settings.json` § `env`, delete one key and change another.
2. `/mcp`.
3. Read the new server's environment (`/proc/<pid>/environ`, or any child it
   spawns).

Both halves of one edit, one reconnect, opposite outcomes.

## Evidence

**The key is genuinely gone from disk.** `grep -c CODESCOUT_QUERY_PREFIX
~/.claude/settings.json` → **0**. JSON re-validated after the edit (21 top-level
keys, `env` 13→12), and `diff` against the pre-edit backup shows exactly the two
intended lines and nothing else.

**The file WAS re-read on reconnect — this is the load-bearing control.**
`CODESCOUT_BM25_BOOST=5.0` exists in no other config layer: `~/.config/codescout/.env`
(→ `.env.gpu`) says `3.0`, both sibling profiles' `.claude.json` say `3.0`, and the
parent process has no such variable at all. `5.0` could only have come from the file
edited moments earlier. So this is **not** a stale-cache-of-everything story; the
reconnect read the new file and still produced the old key.

**It is not shell inheritance.** The parent `claude` process (pid 801487, unchanged
across the reconnect) carries **neither** variable in its own environ — checked
directly. So the value is held in the harness's own in-memory config, not the
process environment it was launched with.

**Fleet snapshot at the time**, showing the new server against its siblings:

| pid | age | prefix | boost |
|---|---|---|---|
| 3995093 (mine, post-reconnect) | 00:11 | **1** | **5.0** |
| 3911498 | 09:10 | 1 | 3.0 |
| 3885828 | 11:32 | 1 | 3.0 |
| 3882799 | 12:02 | 1 | 3.0 |
| 791075 | 06:36:25 | 0 | 3.0 |

## Root cause

**Inferred, not read** — the harness source is not in this repo. The behaviour is
consistent with the spawn environment being built by **merging** the freshly-read
`settings.json` env over a previously-loaded map, rather than replacing it. Under a
merge, a key whose *value* changed is overwritten; a key that no longer *exists*
has nothing to overwrite it, so the stale entry survives.

This is the same shape as the "sourcing cannot unset" hazard already noted in
`docs/trackers/retrieval-benchmark.md`, one layer up: there it was shell env files,
here it is the harness's own config merge.

### The mechanism made a prediction, and it held

The merge hypothesis is not merely consistent with the observation — it forecast a
**different** outcome for a **different** edit, and that outcome occurred.

Prediction: if the harness merges rather than replaces, then re-adding the key with
an empty value should take effect on the next reconnect, because that is an *update*
rather than a deletion — the same operation that already worked for `BM25_BOOST`.

Run: set `"CODESCOUT_QUERY_PREFIX": ""`, `/mcp`, read the new server (pid 4031908,
age 13 s):

```
QUERY_PREFIX = $        # cat -A: empty string, not the old value
BM25_BOOST   = 5.0
```

So the same key that could not be **removed** by a reconnect was successfully
**changed** by one, minutes later, in the same file. That is the update/delete
asymmetry isolated on a single variable, with everything else held constant — which
is stronger than the original observation, where the asymmetry was split across two
different keys and could have been explained by something specific to one of them.

Still **evidence, not proof**: a merge is not the only implementation that would
produce this, and the harness source remains unread. But any competing explanation
now has to account for one key behaving both ways within minutes.
## Why this is worse than a plain stale-config bug

**The half that works falsely confirms the half that does not.** The natural
verification after a config edit is to check the thing you changed. If the edit
contained any update at all, that check passes — and a reader reasonably concludes
the reconnect applied the edit. Nothing distinguishes "the file was re-read" from
"the file was re-read *and fully applied*" except separately probing a key you
*deleted*, which is not an obvious thing to do.

Had this edit changed only the boost, the verification would have been clean and
correct. Had it only removed the prefix, the failure would have been obvious. The
mixed edit is the case that misleads, and mixed edits are the normal case when
tuning a config.

**It also means any config fix expressed as a deletion silently no-ops on
reconnect.** That is the whole class of "remove the harmful setting" fixes — the
form most correctness fixes to an env block take.

## Suspected prior instance

`docs/trackers/retrieval-benchmark.md` § *2026-07-28* records removing
`CODESCOUT_QUERY_PREFIX` from two profiles and marks it **"real — now fixed"**, with
"takes effect on MCP restart". Those two profiles do read clean today, so the fix
did eventually land there. But if it was verified by reconnect rather than by a full
restart, the verification at the time proved nothing, and the month-long survival of
the same setting in a third profile went unnoticed alongside it. **Suspected, not
established** — no record survives of how that check was run.

## Workarounds

- **Full Claude Code restart** for any edit that removes a key. Untested at filing
  time; the next restart is the natural experiment.
- Or **neutralise instead of deleting** — set the key to an empty value if the
  consuming code treats empty as unset. For `CODESCOUT_QUERY_PREFIX` specifically it
  does: `EmbedderHttp::new` reads it with `unwrap_or_default()` and `dense_query`
  short-circuits on empty, so `"CODESCOUT_QUERY_PREFIX": ""` is behaviourally
  identical to absent. **This is the reliable workaround for this key** and it
  survives the merge, because it is an update rather than a deletion.

## Fix

Not ours. The harness should build the MCP spawn environment by **replacing** the
config-derived env rather than merging over the previous one, so that a removed key
is removed.

## Tests added

None — no codescout code is involved.

## References

- `docs/trackers/retrieval-benchmark.md` § *2026-08-30 — D2 resolved* — the edit that
  exposed this, and the fourth config layer it also uncovered.
- `docs/issues/archive/2026-08-17-mcp-reconnect-does-not-refresh-server-instructions.md`
  — same family: `/mcp` refreshes one surface and not another.
- `docs/issues/archive/2026-08-18-clear-leaves-mcp-session-id-stale.md` — same family.
- `docs/issues/archive/2026-08-29-stale-model-dir-env-masked-by-shell.md` — adjacent, but a
  different mechanism (ambient shell masking a repo file, not a harness merge).

---
id: ad69bec0f2cfc31d
kind: bug
status: open
title: buddy plugin's reload-payload from= field can name an unrelated peer session, via last-writer-wins .buddy/.current_session_id
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
opened: 2026-09-03
severity: low
---

## Summary

A peer Claude Code session (`codescout` profile `.claude`, PID 2363194) reported that its `buddy` plugin reload payload this turn carried `from=c95ba99b-17b2-4fe8-a8a5-afed8deb49c6` — this session's sessionId, not the reporting session's own predecessor. The peer traced the mechanism: `.buddy/.current_session_id` is last-writer-wins shared state, and whichever session writes it most recently determines what a *different* session's reload payload names as its own prior identity. Had the peer resolved authorship from that marker instead of independently walking its own process ancestry to its PID and cross-checking the session registry, it would have concluded a set of staged git files belonged to it and attempted to "repair" them on a shared checkout — a genuine misattribution with a destructive-action consequence.

## Symptom (Effect)

A session's `buddy`-plugin reload/compaction payload can name a **different, unrelated session's** sessionId as `from=`, with no error, no warning, and no signal that the value is stale or foreign. The consuming session has no built-in way to distinguish "this is genuinely my prior turn" from "another session overwrote the shared marker file after mine wrote it and before mine read it back."

## Reproduction

**Reproduced 2026-09-08** (see § *Verified at the bytes*). The shape the original filing guessed is the right one: two Claude Code sessions active in the same checkout, both using the `buddy` plugin; session A writes `.buddy/.current_session_id`; session B writes it after; session A later triggers a compaction/reload cycle and reads back session B's write.

## Environment

Shared checkout at `/home/marius/work/claude/codescout` (and any other shared checkout running the `buddy` Claude Code plugin across multiple concurrent sessions/profiles). Not a codescout-repo code defect — `.buddy/` is plugin state, not part of this repository's own source.

## Root cause

*Filed second-hand; **independently verified at the bytes 2026-09-08** — see the section below.* Per the peer's original trace: `.buddy/.current_session_id` is written by whichever session touches it most recently, with no per-session scoping and no staleness check. This repo's own `codescout-companion:reconnaissance` skill already documents the general shape of this hazard in a different context — its statusline-marker instructions say *"`$CLAUDE_CODE_SESSION_ID` first — the order is load-bearing, because the statusline resolves the sid from the harness while `.current_session_id` is last-writer and can name a peer"* — but that guidance covers codescout's own recon statusline marker, not the `buddy` plugin's reload-payload `from=` field, which is a separate mechanism the peer traced independently.


### Verified at the bytes, 2026-09-08 — and the harness value was in hand the whole time

A second session (`59112612-5fc8-4b31-8c8c-e19220d99eac`) hit this after a compaction, traced it
through the plugin source, and measured the on-disk consequence. Independently corroborated by a
third session working in the plugin repo, which had written the same trace line for line.
The mechanism is confirmed, and it is one indirection deeper than "the payload reads the pointer".

**The pointer is read into an environment variable, at `hook_entry.py`:**

```python
# Capture previous session id BEFORE overwriting the pointer (reload uses it).
prev = ""
pointer = buddy_dir / ".current_session_id"
if pointer.is_file():
    prev = pointer.read_text().strip()
os.environ["BUDDY_PREV_SID"] = prev
```

The comment is correct on a **single-session** checkout, where the pointer's last writer
necessarily *is* your predecessor. That is what makes it fail silently rather than loudly: the
abstraction is exactly right until a second session exists.

**`handle_session_start` then treats that value as lineage:**

```python
if prev_sid and prev_sid != incoming_sid:
    prev_state_path = project_root / ".buddy" / prev_sid / "state.json"
    if prev_state_path.is_file():
        prev_state = load_state(prev_state_path)
        carried_specialists = list(prev_state.get("active_specialists", []) or [])
        if carried_specialists:
            state["active_specialists"] = carried_specialists
        state["parent_sid"] = prev_sid
else:
    carried_specialists = list(state.get("active_specialists", []) or [])
```

**Measured, and the two effects are not equally severe.** Distinguish them:

| effect | status |
|---|---|
| `state["parent_sid"]` set to a live peer | **landed**, on disk, every shared-checkout compaction |
| a peer's `active_specialists` adopted | **armed, did not fire** — the peer's list was `[]` |

The reporting session's `.buddy/<sid>/state.json` recorded `parent_sid: 5399543d-…`, which
resolved by socket enumeration to a live peer in this same checkout — not a predecessor. The
specialist line was reached; only the inner `if carried_specialists:` guard held. That is a
data-dependent near miss and must not be reported as an observed transfer.

**On `source=compact` the sessionId is PRESERVED**, which is the fact that makes the fix one
branch. Verified independently of the plugin: the reporting session's git `Session-Id` trailers
are byte-identical on commits either side of its compaction, and `CLAUDE_CODE_SESSION_ID` in its
shell matched both. So `prev_sid` is `incoming_sid` by definition on a compact, the `else` branch
is the correct one unconditionally, and **the pointer should never be read at all** for that
source. `hook_entry.py` computes `sid = _session_id(event)` from the harness two lines above the
pointer read — the composer held the right input and used a different one.

For `resume`/`fork` a genuine predecessor exists and the pointer is still the wrong source; it is
merely right more often, because a resume usually follows the resumed session's own last hook.
Where the harness supplies no parent id, **recording nothing beats recording a peer** — an absent
`parent_sid` is legible, a wrong one is not.

**Why the affected party cannot catch it.** A compacted session has no independent memory of its
own predecessor — that is what compaction removes — so `from=<peer>` is unfalsifiable from inside
the one context that reads it. It surfaced only through an unrelated instrument: this repo stamps
a `Session-Id` trailer on every commit, so `git log` holds a record the plugin does not control.

### Third instance, 2026-09-12 — a confirmation, plus one refutation route this file said did not exist

Session `05841db2-4ba0-4cb2-a22f-c0bc2f771e20` compacted and its payload read
`from=f3c594ce-c424-40d3-a603-9693cfef3f63`. Socket enumeration at 19:26:53 returned 18 live
sessions across 3 profiles, 5 of them in this checkout, and `f3c594ce` was one of them —
PID 703051, name `attach-alias-advisory-anyhow`, status `waiting`.

**Reported as a confirmation, not a catch.** Both effects landed exactly as the table above
predicts: `.buddy/<sid>/state.json` recorded `parent_sid: f3c594ce-…`, and
`active_specialists` was `[]` on both sides, so the adoption path was reached and did not
fire. That is now **two independent near misses and zero observed transfers** — a
denominator. The severity of the armed effect rests entirely on how often a session holds a
non-empty specialist list, which neither instance measures.

**New: the pointer's churn was observed directly, and the recorded lineage is a chain of
contemporaries.** `.buddy/.current_session_id` was read twice about three minutes apart and
held two *different* peer sids — `8bd791df-…` (`codescout-1a`, PID 924391) at ~19:27, then
`f3c594ce-…` at ~19:30. Following the recorded parents: this session's `parent_sid` is
`f3c594ce`, and `f3c594ce`'s own `parent_sid` is `8bd791df`. All three were alive at the same
instant. The on-disk lineage is not a stale ancestor chain that merely decays — it is a chain
of concurrent peers, re-pointed at whatever rate sessions start and compact.

**And the "unfalsifiable from inside" claim above needs one narrowing.** It is right that a
compacted session has no memory of its predecessor. But it does not need one: process start
time refutes the lineage directly. PID 703051 started 2026-09-11 09:09:18, **7h47m before**
the reading session's own process (PID 2706008, 16:56:18), and both transcripts were being
appended within two minutes of the check (19:26:26 and 19:28:31). A predecessor cannot both
predate you by seven hours and still be writing. This is cheaper than the `Session-Id`
commit-trailer route this file names, needs no commits to exist, and works on the first turn
after a compaction — so the accurate statement is that `from=` is unfalsifiable **from
context**, not unfalsifiable from **inside the session**.

### Fourth instance, 2026-09-13 — self-refuting inside the banner, and the first near miss with a DISCRIMINATOR

Session `05841db2-4ba0-4cb2-a22f-c0bc2f771e20` (`codescout-aa`, profile `.claude-kat`, PID 2706008)
compacted at ~09:30 and its banner read:

```
sid=05841db2-…  from=b80a27d4-…  source=compact
payload-file=.buddy/05841db2-…/reload-payload-compact.md
```

**Positive identification, not elimination.** `b80a27d4-9729-40ef-8c28-ad8982df6d13` resolves
through `/home/marius/.claude-sdd/sessions/872862.json` to name `codescout-87`, profile
`.claude-sdd`, cwd this checkout, status `idle` — a live peer. Recorded lineage this time is
`05841db2` → `b80a27d4` → `b0b9bc40-…` (`.claude-sdd`, PID 1456596), and **both ancestors sit on a
profile that is not the reading session's**, so no `ListAgents` call from inside it could ever have
surfaced either. Instance 3's chain-of-contemporaries finding holds with a fresh chain and a new
edge: the pointer crosses profile boundaries, because the checkout is what it is shared by.
The churn was caught again — `.buddy/.current_session_id` held `b80a27d4` at compact time and
`05841db2` at 09:54:25, rewritten mid-turn.

**The cheapest refutation yet, and it needs nothing outside the banner.** Instance 3 narrowed *"un-
falsifiable from context"* to *"falsifiable from inside, via process start time"* — which still
requires resolving the named peer's PID. Cheaper: **the banner contradicts itself on one line.**
`payload-file` is under the READING session's own sid while `from=` names another. For
`source=compact` the payload is read from your own directory, so any `from=` naming a different sid
disagrees with the path printed beside it. Zero lookups, available on the first turn, and it still
works after every named peer has exited.

A second route, independent and nearly as cheap: this session's own pre-compaction commits carry
`Session-Id: 05841db2` (`23adef79`, `02e61230`, `96574bfa`), so **the sid survives compaction** and
the "previous session" for `source=compact` is you. That refutes any differing `from=` without
resolving the named peer at all — where the commit-trailer route as this file previously described
it checks the NAMED session's commits.

**First instance carrying a DISCRIMINATOR rather than an absence — and the prior readings were
monotone under the very thing they excluded.** Instances 2 and 3 recorded `active_specialists: []`
on both sides and read it as *"the adoption path was reached and did not fire"*. `[]` on both sides
cannot separate that from *"it fired and copied an empty list"*: the observation is identical under
both, which is § *Testing Discipline*'s monotone law holding against this file's own evidence.

Here the two sides differ observably. `loaded_skills.json` holds **3** skills for `05841db2` and
**5** for `b80a27d4`, three of them (`writing-plans`, `subagent-driven-development`,
`using-git-worktrees`) unique to the peer. The reload restored `reconnaissance` — present in the
reading session's own list at `first_ts 1789149189` — and **none** of the peer's three unique
skills. So the payload's CONTENT is the reading session's and only the `from=` attribution is
wrong, which is now shown rather than inferred.

`active_specialists` was `[]` on both sides again, so the tally is **three near misses, zero
observed transfers** — but this is the first where a transfer would have been VISIBLE had it
occurred, so it is the first that is evidence about the mechanism rather than about the sample.
**The armed effect is still unmeasured in the direction that matters:** nothing here exercises a
non-empty `active_specialists`, and severity rests entirely on how often a session holds one.
## Evidence

Quoted from the peer's cross-session message (2026-09-03):

> "my buddy reload payload this session carried `from=c95ba99b-19b2-4fe8-a8a5-afed8deb49c6` — your sessionId, not my predecessor's. Had I resolved authorship from that marker instead of walking my own process ancestry to pid 2363194 and reading the registry, I would have concluded the staged files were mine and 'repaired' them. That is the documented `.buddy/.current_session_id` last-writer hazard showing up in the artifact best placed to cause a misattribution."

## Hypotheses tried

The original filing ran none — it rested on a peer's self-reported trace. **The 2026-09-08 verification tested and settled three things**, recorded in § *Verified at the bytes*: that the sessionId is preserved across a compaction (so `from=` should equal `sid` on that source), that the composer holds the harness value it declines to use, and that the specialist-adoption path was reached but did not fire. One reading was tried and **rejected**: that the payload reads the pointer directly. It does not — the pointer is read in a different hook and passed through the `BUDDY_PREV_SID` environment variable, which is why a grep of the payload-emitting script finds nothing.

## Fix

Not designed, and not this repo's code to fix (the `buddy` plugin is external). Candidate direction, per the peer's own framing: the reload payload's `from=` field should be resolved from a per-session-scoped source (analogous to `$CLAUDE_CODE_SESSION_ID`), not from the shared last-writer `.buddy/.current_session_id` file, or should at minimum be validated against the harness-provided session id before being trusted for an authorship decision.

## Tests added

None — no code in this repository implements the mechanism described.

## Workarounds

**Check the banner against itself first — it costs nothing and needs no peer to still exist.** For
`source=compact` the payload is read from the reading session's own directory, so `payload-file`
and `from=` must name the same sid. When they disagree, `from=` is wrong and you are done; no
registry lookup, no `/proc` walk, no live peer required. Second-cheapest, and independent: your own
pre-compaction commits carry `Session-Id: <your sid>`, and the sid survives compaction — so for
`source=compact` the "previous session" is you, and any other value is refuted without resolving
whoever it names.

Only if both are unavailable, fall back to the expensive routes: process start time against the
named peer's PID (instance 3), or resolving the sid through
`$CLAUDE_CONFIG_DIR/sessions/<pid>.json` across **every** profile — the named session is routinely
on a different one (instance 4), so a single-profile scan returns a plausible "not found".

Either way, a session using `buddy`'s reload payload for an authorship decision must cross-check
rather than trust `from=` alone, exactly as
`docs/conventions/shared-checkout-commit-sequence.md` already prescribes for staged-file authorship
generally.
## Resume

1. ~~Independently reproduce or further trace the mechanism~~ — **done 2026-09-08**, see § *Verified at the bytes*. The plugin source was read; the mechanism, the two effects and the one-branch fix are all named there. What remains is a fix in the `buddy` plugin's own repo, which is not this repository's code.
2. **Classified 2026-09-03 as `IC-12` (`cluster/transient-shared-state-lies-to-readers`)**, per a peer session's discriminator (`ffb95976`): the remedy here is to stop trusting a piece of shared state, not to build a new reporting instrument for an unreported event, which rules out `IC-1`. Tag applied through the catalog and `+1:` appended to `IC-12`'s Members field in the same commit.
3. No further ledger action owed for this bug.

## References

- Peer session report: codescout session `66523284-814b-49b8-b8f8-820dc2b00be2`, PID 2363194, profile `.claude`, 2026-09-03
- `docs/conventions/shared-checkout-commit-sequence.md` — the general discipline this instance is a specific case of (identify authorship positively, never by inference)
- `docs/trackers/issue-clusters.md` — candidate home once classified (see Resume step 2)

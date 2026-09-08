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

A session using `buddy`'s reload payload for an authorship decision should independently cross-check via process ancestry and/or the session registry (as the reporting peer did) rather than trusting `from=` alone, exactly as `docs/conventions/shared-checkout-commit-sequence.md` already prescribes for staged-file authorship generally.

## Resume

1. ~~Independently reproduce or further trace the mechanism~~ — **done 2026-09-08**, see § *Verified at the bytes*. The plugin source was read; the mechanism, the two effects and the one-branch fix are all named there. What remains is a fix in the `buddy` plugin's own repo, which is not this repository's code.
2. **Classified 2026-09-03 as `IC-12` (`cluster/transient-shared-state-lies-to-readers`)**, per a peer session's discriminator (`ffb95976`): the remedy here is to stop trusting a piece of shared state, not to build a new reporting instrument for an unreported event, which rules out `IC-1`. Tag applied through the catalog and `+1:` appended to `IC-12`'s Members field in the same commit.
3. No further ledger action owed for this bug.

## References

- Peer session report: codescout session `66523284-814b-49b8-b8f8-820dc2b00be2`, PID 2363194, profile `.claude`, 2026-09-03
- `docs/conventions/shared-checkout-commit-sequence.md` — the general discipline this instance is a specific case of (identify authorship positively, never by inference)
- `docs/trackers/issue-clusters.md` — candidate home once classified (see Resume step 2)

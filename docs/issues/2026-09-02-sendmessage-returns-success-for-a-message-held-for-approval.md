---
id: '908d1a59914b0785'
kind: bug
status: open
title: 'BUG: SendMessage returns success: true for a message that was held for the recipient user''s approval and never delivered'
tags:
- cluster/record-asserts-an-unchecked-completion
- multi-session
- shared-checkout
- coordination
- harness
opened: 2026-09-02
owner: marius
related:
- docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
- docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md
severity: medium
unverified: Root cause is inferred from the observable pair, not read in harness source — the mechanism is not readable from this repo. Also unknown whether a never-actioned message ever produces a notice; see Resume.
---

## Summary

`SendMessage` returns `{"success": true, "message": "… → <name> (another Claude session on this machine)"}` for a message that was **not delivered** — it was held pending the recipient user's approval. The `[Cross-session delivery notice]` saying so arrives **asynchronously, one or more turns later**. A sender that reads the return value as delivery proceeds on a false premise, and there is no synchronous signal that distinguishes *accepted for delivery* from *delivered*.

## Symptom (Effect)

Send returns, verbatim:

```
{"success":true,"message":"“Correct commit-count attribution; report plan amendment” → codescout-20 (another Claude session on this machine)","msg_id":"d1295297-d151-40af-8f00-4d4ef9dbe9bd"}
```

Some turns later, unprompted:

```
[Cross-session delivery notice] Your message to another session was held for the
recipient user's approval (recipient: uds:/run/user/1000/cc-socks/3857171.sock).
Not delivered to that session's Claude yet; its user must approve first.
Do not wait for a reply; continue, or choose another approach.
```

Note the return carries `success: true`, an arrow (`→ codescout-20`) reading as delivery, and a `msg_id`. Nothing in it is hedged, and no field distinguishes queued from delivered.

## Reproduction

`git rev-parse HEAD` at observation: `2e40fba9`, branch `experiments`.

1. From session A, `SendMessage(to: "<name-or-uds-path>", message: …)` to a session whose user has inbound peer-message approval enabled.
2. Read the return: `success: true`, delivery-shaped string, `msg_id` present.
3. Continue working. The hold notice arrives on a later turn.

Observed twice in one session against the same recipient (PID 3857171, `codescout-20`, sessionId `19e0e253`). Messages to two other peers in the same session were delivered normally — one replied — so the behaviour is per-recipient-user configuration, not a global outage.

## Environment

Linux, Claude Code 2.1.258, three config profiles (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`), 21 live sessions across them at time of observation. Sender profile `~/.claude`, sessionId `ffb95976`. Recipient same profile, addressable by bare name.

## Root cause

Unknown at the harness level — `SendMessage` is a harness tool, not codescout's, so the mechanism is not readable from this repo. **Inferred from the observable pair, not measured in source:** the return value is generated at enqueue time and the approval gate is applied at the recipient, so the two are necessarily separated by the approval interval. What is *established* is the observable contract: a `success: true` return does not imply delivery, and no field in the response indicates which happened.

## Evidence

### The two delivery notices, same recipient

```
[Cross-session delivery notice] Your message to another session was held for the recipient user's approval (recipient: uds:/run/user/1000/cc-socks/3857171.sock). Not delivered to that session's Claude yet; its user must approve first.
[Cross-session delivery notice] Your message to another session was held for the recipient user's approval (recipient: uds:/run/user/1000/cc-socks/3857171.sock). Not delivered to that session's Claude yet; its user must approve first.
```

### The cost, in this session

An advance announcement of a mutation window was sent to `codescout-20` before mutating `src/librarian/indexer.rs` on a shared checkout. The purpose was precisely so that a red build observed during the window would have a known author rather than being the anonymous kind recorded in `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md`.

The announcement was never delivered. On the strength of the `success` return, the following was written into `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` and **committed at `8dee33a8`**:

> The mutation window was announced to the one peer sharing this tree beforehand, so a red observed inside it would have had an author rather than being the anonymous kind…

That sentence was false when committed. Corrected in place afterwards; the correction is the interesting artifact, not the error.

## Hypotheses tried

1. **Hypothesis:** the recipient session had exited, so the name failed to resolve.
   **Test:** `scripts/peer-sessions.sh` plus `~/.claude/sessions/3857171.json`.
   **Verdict:** rejected — PID 3857171 live, `status: idle`, name `codescout-20`, sessionId `19e0e253` unchanged. And the notice itself says *held for approval*, not *unreachable*.
2. **Hypothesis:** all cross-session messaging was down.
   **Test:** two messages to `codescout-dd` (`uds:/run/user/1000/cc-socks/1754443.sock`) in the same session.
   **Verdict:** rejected — both delivered, both answered with substantive replies.

## Fix

None in this repo — the tool is the harness's. **The remedy is a documentation and practice change**, and it is the one that generalises:

> **A `success` return from `SendMessage` means *accepted for delivery*, not *delivered*. Treat a peer as uninformed until it replies.**

That matters most for the case the announce channel exists to serve: announcing a mutation window, a rebase, or a shared-tree operation. Those are exactly the sends whose value depends on arrival, and exactly the ones where the sender proceeds immediately.

`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` § *Instance 5* is titled "the captured side, and an announce channel that was used and did not help". That instance recorded the symptom; this file names a mechanism sufficient to produce it.

## Tests added

None — the behaviour is in the harness, not in codescout, and there is no seam here to test against. Recorded rather than tested, deliberately.

## Workarounds

- **Do not treat an unanswered announcement as delivered.** If the coordination is load-bearing (mutation window, rebase, force-push), either wait for an explicit reply or proceed as if unannounced — and say which you did.
- A hold notice names the recipient by `uds:` path. Resolve it to a session with `scripts/peer-sessions.sh` before assuming which peer did not hear you.

## Resume

Open question, not yet investigated: does the hold notice arrive at all if the recipient's user never acts — or only on approval/denial? If a never-actioned message produces no further notice, then a sender's silence is ambiguous between *held forever* and *delivered but unanswered*, which would make the workaround above insufficient. Test by sending to an approval-gated session whose user is known to be away, and observing whether any notice arrives within a bounded interval.

## References

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` § *Instance 5*
- `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md`
- `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` § *Task 6* Step 5 (carries the correction)
- `CLAUDE.md` § *Reaching a Peer Session*


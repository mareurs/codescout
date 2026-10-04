---
id: '03715066b89fc596'
kind: convention
status: active
title: Observer blindness — when care is the wrong instrument, the long form
owners:
- marius
tags:
- observer-blindness
- conventions
- epistemics
topic: defect classes the right party cannot see
---

# Observer blindness — the long form

`CLAUDE.md` § *Observer Blindness* points here. This page keeps the section's full text, including the measurements and incident accounts that sit inside it, moved here unchanged on 2026-10-03. The class ledger is [`docs/trackers/observer-blindness.md`](../trackers/observer-blindness.md).

Some defect classes are invisible to the party best placed to catch them **by construction**, and
they return a **plausible answer rather than an error** — so nothing downstream fires either. For
these, "be careful" is not a weak remedy, it is the **wrong instrument**. Measured 2026-08-30: four
instances of one class in one evening across three sessions, and **every one was committed by an
author actively writing about that class** — one reintroduced a bare integer in the commit fixing a
bare integer, ten minutes after withdrawing an ordinal for identical reasons. Knowing the class
prevented none of the four. A standing policy caught one.

**So when you meet one, do not resolve to check harder — name three things and build the third.**

1. **Who structurally cannot see it, and the reason.** "Was careless" disqualifies it; "holds the
   parameter that would reveal it" qualifies it.
2. **Who can.** This predicts the **reviewer**, and it is one who does *not share the author's
   context* rather than a more careful one — which is why self-review is structurally unavailable
   here, and peer review is a different instrument rather than a redundancy.
3. **The check that runs when nobody is worried.** Best shape is making the correct path end in a
   safe state, so compliance leaves nothing armed (`73066479` — the lean lane runs third precisely
   so the gate cannot be *followed correctly* and still arm the next session). Next best is an
   unconditional policy tied to a trigger that happens anyway. And for any published claim, ship its
   **derivation** rather than its value, so a reader re-checks it instead of re-deriving it under a
   counting rule of their own choosing. **And ship its POPULATION in the same place.** A bound
   that lives in the *enforcement* layer — a test module header, a gate script, a hook — is
   correctly published to an audience that never reads the number, and the author cannot perceive
   the gap because they are the party holding the bound. So when a tracker's number and its scope
   live apart, the fix is to **move the scope to the read surface**, not to record the lesson:
   publishing again is redundant and reading harder is impossible, since the reader does not know
   the other surface exists. Worse, a document that carefully names *one* failure mode implies by
   omission that the rest are handled. (`OB-1` § *the third position*,
   `reconnaissance-patterns:R-170`: a 29.5% tag-coverage ratio read as drift, one step from a
   236-file campaign the gate's own header forbade in writing. Cheap tell — **a coverage ratio
   that is neither ~0% nor ~100% is a boundary someone drew before it is drift**; and before any
   campaign over a population, grep `tests/`, `scripts/pre-commit-*` and hooks for that
   population's name, not only the docs.)

**Authorship on a shared checkout is one of these.** The operational procedure — the scope table,
the skill to invoke, the addressing forms, the unit rule — is § *Reaching a Peer Session* above, and
that is the copy to follow. What belongs here is only the epistemics: why the obvious instrument is
the wrong one.

**Never close an authorship question by elimination — identify positively.** Elimination is sound
only over a population **proven complete by an instrument that spans the whole namespace**, and two
agreeing instruments are not that when they share a scope. Two instruments that share a blind spot
agree *because* of it — one blind spot counted twice, and the shape is indistinguishable from real
agreement at the point of use. **So completeness is the thing to check, not the inference.** A
windowed instrument's zero is scoped to its window, and re-running it later silently moves that
window; the positive identifier for uncommitted state is to resolve the session's own registry row
from the socket its message arrived on — a channel the sender does not control (§ *Reaching a Peer
Session* holds the route). **Asking it to quote its scratchpad path is the FALLBACK, and calling
that *given* was too strong.** The harness does make the session id a path component, but a session
reporting its own path is still reporting, and nothing ties the quoted string to the process that
sent it. Both beat inference; only the channel route is independent of the message body.

**That holds for the sessionId and fails for the NAME — and the name is what sessions actually
quote at each other.** A name is minted into a per-profile registry
(`$CLAUDE_CONFIG_DIR/sessions/<pid>.json`); compaction, resume, or a restart under another profile
mints a new one and nothing re-informs the running context, so a session reporting its own name is
quoting a belief rather than reading a fact. Measured 2026-09-02, twice in one evening: a peer
signed as `codescout-26` — a session that had already exited on another profile — and
`bug-fix-session-log:F-97` recorded the misattribution before that peer corrected it by reading its
own registry entry; separately `codescout-00` became `codescout-cc` on a different profile and PID
with its sessionId unchanged, which is the only reason earlier stage-log attributions kept
resolving to it. **So attribute by sessionId, never by a self-reported name.** The name is what
`ListAgents`, `SendMessage` and the socket table all display, which is exactly why the substitution
is easy to make and hard to notice.

Record classes as `OB-N` in
[`docs/trackers/observer-blindness.md`](docs/trackers/observer-blindness.md) (artifact
`3922c2a0fd0dfcfc`). The admission tests, the field block, the mining greps, and the full measured
history of every instance — including the corrections that superseded earlier readings — live in the
file; the one-line index is [`docs/TAXONOMY.md`](docs/TAXONOMY.md). **An instance is a bug file, an
`F-N` or an `R-N`; only the class is an `OB`.** A row reading `**Mechanism status:** none yet` is a
design worklist item, and is exactly what `H-N` (hooks) and `I-N`
([`docs/trackers/test-escape-hardening.md`](docs/trackers/test-escape-hardening.md)) consume — that
tracker reached the same conclusion from the cost side, *"lenses must move LEFT into standing
mechanisms so they catch by default without a human remembering"*, and the two are complements
rather than copies.

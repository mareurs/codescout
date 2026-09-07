---
id: '79674d2d77f15d8b'
kind: bug
status: open
title: 'BUG: the source-file redirect names three tools this session does not have, and its breaker cannot stand down because codescout is answering'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
- companion-plugin
- hooks
- tool-surface
topic: guards whose remedy text assumes a tool set the caller does not have
---

## Summary

`pre-tool-guard.mjs` denies native `Read` and `Edit` on source files and redirects to
`read_file`, `edit_file` and `edit_code`. In this session **none of those three tools exists** —
the codescout MCP server is connected and answering, but exposes a *subset* of its tools. So the
redirect names a route the caller cannot take.

Its `ESCAPE_FOOTER` then supplies a cause: *"the MCP server has disconnected and this redirect
points at nothing."* That is **false here and checkably so** — `doc`, `grep`, `run_command`,
`symbols` and `semantic_search` all answered in the same turn as the deny.

The breaker that exists precisely to rescue this state **cannot fire**, and for a reason that is
the interesting part rather than an oversight: it stands down after `BREAKER_THRESHOLD = 3`
consecutive denies *with no codescout tool answering in between*. A partial tool set is the exact
state where codescout answers constantly, so the strike count resets on every call and never
reaches three. **The guard's rescue is keyed on total silence; the defect is partial presence.**

## Symptom (Effect)

Two denies, in one session, both reproduced:

- Native `Read` on `src/librarian/entry_token.rs` → *"codescout has a faster path for source
  files"*, redirecting to `symbols` (present) and `read_file` (**absent**).
- Native `Edit` on the same file → *"codescout's edit_code is the safer path for structural
  source edits"*, redirecting to `edit_code` (**absent**) and `edit_file` (**absent**).

Between them there is no available path to the intended tool. The escape that does work —
`run_command` with `acknowledge_risk: true` — is named nowhere in either message. It was found by
reading the *other* gate's refusal (IL-3's source-file block, which does advertise its bypass).

**So the harm is not the deny; it is the twenty tool-calls of misrouting the deny invites.** A
reader who trusts the footer goes to `/mcp` to fix a connection that is not broken.

### Instance 2 — 2026-09-07, an INDEPENDENT trigger, reported by the session that was structurally blind to it

`cda3afe5-17b8-4863-9f4c-9fe4eadbc17b` told this session an hour earlier that its own tool set
was complete, so the footer could never name a tool it lacked — correctly, at that instant. It
then went **partial mid-conversation**: `read_file`, `edit_file`, `create_file` and `edit_code`
disappeared while `run_command`, `grep`, `symbols`, `doc`, `tree`, `librarian` and `memory`
remained. Native `Read` on a `scripts/*.sh` file produced the same deny and the same footer, and
they falsified it **in the same turn**: `run_command` answered and reported the server pid.

Three things this adds that instance 1 could not:

- **The trigger is independent.** Instance 1's partial set was the state this session began in;
  instance 2's arrived from a session/profile change mid-conversation. Two routes to one state is
  worth more than two reports of one route — and it is *independence*, not agreement, that makes
  the second observation evidence.
- **The lost subset is not arbitrary.** Roughly 14 tools stayed and **4 went**, and the 4 are
  precisely the ones the source-file and markdown redirects name. So the footer is not merely
  wrong about a disconnected server; it is wrong about **exactly the subset that goes missing**,
  which is what makes *"is codescout up?"* the wrong question to key on rather than a coarse one.
- **The remedy is wrong, not only the diagnosis.** *"ask the user to run `/mcp`"* is the next
  action the footer produces, and it points at a working component — a reader who follows it
  interrupts their operator to repair nothing. That is `OB-20`'s shape (a guard whose predicate
  fires correctly and whose message sends you somewhere useless), and it is the half a reworded
  footer would fix. `IC-2` stays the primary tag for the reason in § *Class*: rewording leaves the
  breaker unreachable.

Recorded as corroboration rather than shared authorship, at the reporting session's own request.

## Reproduction

1. Run a session where codescout is connected but `read_file` / `edit_file` / `edit_code` are not
   in the exposed tool list. (Observed as the default in this session; not deliberately
   configured by me.)
2. Call native `Read` on any file under `src/`. → deny + redirect to `read_file`.
3. Call `read_file`. → `No such tool available: mcp__codescout__read_file`.
4. Call any codescout tool that *is* present. → answers normally, and resets the breaker.
5. Repeat 2–4 indefinitely. The stand-down at three strikes is unreachable.

Step 4 is the load-bearing one: ordinary work *is* step 4, so the breaker is reset by the
session doing its job.

## Root cause

The hook cannot observe the caller's tool list — a `PreToolUse` payload does not carry it — so it
substitutes a proxy for *"is my redirect target reachable?"*: **proof-of-life, "has any codescout
tool answered since the last deny."**

The proxy is sound for the case it was built for (server gone → nothing answers → stand down) and
wrong in exactly one direction: **tool present ⊅ tool set complete.** Both the footer and the
stand-down text state the collapsed premise outright — the stand-down calls disconnection *"the
usual cause"* of an unanswered redirect and describes it as stripping *"every codescout tool"*,
which is the total case named as though it were the only one.

Sites, in `../claude-plugins/codescout-companion/hooks/pre-tool-guard.mjs` (at the plugin's
2026-09-05 state): `ESCAPE_FOOTER` (~L68), `BREAKER_THRESHOLD` (~L61), the stand-down branch
(~L132), the `Read`-on-source `enforce` (~L277), the `Edit`-on-source `enforce` (~L310).

## Class

`cluster/gate-keyed-on-unobservable-event` (`IC-2`) — the gate keys on an event it cannot see and
substitutes a proxy that fails silently.

**It also lands on `IC-22`'s sixth-member seam** (*hint composed without the request*, extended
there from next-step hints to **causal explanations**), and the two readings pick different
fixes, which is why the tag is not a coin-toss: `IC-22` would have you reword the footer;
`IC-2` says the wording is downstream of a proxy that cannot distinguish partial from total
absence. Rewording alone leaves the breaker unreachable.

The footer's own comment shows the author reasoning correctly about an *adjacent* case — it
deliberately does not point at `.claude/codescout-companion.json`, because the guard denies
writes to that file too, "so the documented block_reads escape is unreachable by whoever is being
blocked." The same sentence, applied one step further, is this bug.

## Hypotheses tried

1. **"The plugin in the repo is not the running copy."** Falsified — the strings are present in
   the source repo's `hooks/pre-tool-guard.mjs`. An initial `grep` for the footer text against
   that file returned nothing and looked like confirmation; it was the *first* grep being scoped
   to a single-word pattern the constant wraps across lines. Recorded because the false negative
   was quiet and pointed at a cache-drift story that would have been wrong.
2. **"codescout is partly disconnected."** Falsified by the probe: every present tool answered in
   the same turn, before and after each deny.

## Fix

Not fixed here — it is the companion plugin's repo, and the remedy is a design choice for its
owner. Three options, cost ascending:

- **Say less, accurately.** Replace the footer's asserted cause with the two states the caller can
  actually distinguish: *"if these tools are not in your list, either codescout is disconnected
  (run `/mcp`) or this session exposes a subset — in which case `run_command` with
  `acknowledge_risk: true` is the escape."* Names a route that works in both branches.
- **Make the breaker key on the right event.** Count a strike per deny and clear it only when a
  *redirect target* is called, not when any codescout tool answers. Turns proof-of-life into
  proof-of-reachability, which is the thing being proxied.
- **Let the hook learn the tool set.** Most work; may not be available from a `PreToolUse`
  payload at all, which is why the proxy exists.

Recommendation: the first two, together. The first is cheap and removes the misrouting; the
second restores the rescue the breaker was written to provide.

**Do not "fix" this by dropping the footer.** It is the only sentence in the deny that tells a
caller the redirect might be unreachable at all; the defect is its confident single cause, not
its presence.

## Tests added

None — the code is in another repo. The assertion shape is worth stating because it is the one
this corpus keeps getting wrong: **do not pin the footer's prose.** Assert that the message names
**a route reachable under a partial tool set**, and separately that a deny **increments a strike
that a non-target codescout call does not clear**. The first reds on a rewrite that drops the
escape; the second reds on the actual defect. A test that only asserts the deny fires is monotone
under all of this — it passes today, and it passed throughout.

## Workarounds

`run_command(..., acknowledge_risk: true)` reaches source files for both reading and writing. Used
in `60809465` to re-point one doc comment. It is a documented bypass advertised by the *other*
gate, not a hole.

## Resume

Owner decision on which of the three fixes; then the two assertions above in the plugin repo.

## References

- `../claude-plugins/codescout-companion/hooks/pre-tool-guard.mjs` — all five sites.
- `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md` — the class.
- `docs/trackers/issue-clusters/IC-22-hint-composed-without-the-request.md` — the secondary
  reading, and the member that extended it to causal explanations.
- `CLAUDE.md` § *Testing Discipline* — *"an alarm can fire, be read by exactly the right person,
  and send them somewhere useless, because a suite tests a guard's PREDICATE and never its REMEDY
  TEXT."* This is that, with the twist that the remedy's own rescue mechanism is keyed on the
  same false premise as its text.
- `CLAUDE.md` § *Companion Plugin* — *"never infer a tool's availability from this file — call the
  tool once instead."* Correct advice, and it is what established the finding.
- Found by sessionId `4eac25ba-b181-4dac-a5a1-ec88502a5bc5` while re-pointing a citation during
  the archive move in `60809465`.

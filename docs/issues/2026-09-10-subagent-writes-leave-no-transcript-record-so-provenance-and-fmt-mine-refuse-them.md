---
kind: bug
status: investigating
tags:
- cluster/guard-narrower-than-its-name
- provenance
- sdd
- fmt-gate
closed: null
opened: 2026-09-10
owner: marius
related:
- docs/issues/archive/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md
- docs/issues/archive/2026-09-08-the-provenance-selector-kept-the-pre-rename-tool-name.md
severity: high
---

# BUG: subagent writes produce no transcript record, so `file-provenance.py` returns UNKNOWN for every file an SDD task writes — and `fmt-mine.sh`, the gate's first command, cannot format any of them

## Summary

`scripts/file-provenance.py` reads Claude Code transcripts to attribute a file to the session
that wrote it. Subagent tool calls do not appear in those transcripts at all — there are **zero**
`isSidechain: true` records — so every file written by a subagent attributes to nobody.
`scripts/fmt-mine.sh` consumes that verdict and formats only what it can attribute, so the
**mandatory first command of this project's gate cannot format the normal output of this
project's own `subagent-driven-development` skill — and, measured 2026-09-10, cannot be run at
all by any OTHER session on the checkout while such work is dirty** (see *The refusal blocks
every other session* below). The script's own comment claims to handle
sidechain records, which is a documented behaviour the substrate cannot support.

**Scoped 2026-09-10, correcting this file's own first draft.** The blindness is total — provenance
sees *no* subagent write — but the **refusal is conditional**: `fmt-mine.sh` runs
`cargo fmt -- --check` as stage 1 and `exit 0`s there when nothing would be rewritten
(`scripts/fmt-mine.sh:34-36`, `:80-85`), which its own comment calls *"the overwhelmingly common
case"*. So the gate only refuses when a subagent-written file **actually needs reformatting**.
The first draft said *"refuses on all of them"*, which overstated the frequency while getting the
mechanism right. Witnessed both ways inside one task: SDD Task 1's implementation round hit the
refusal and fell back to `cargo fmt`; its fix round exited 0 with no fallback, because that
round's edits needed no reformatting. **Severity stays high** — when it bites it blocks the gate's
first command with no owner to ask and no `--force` — but a reader sizing the exposure should
expect it on *some* task rounds, not all.

## Symptom (Effect)

Observed 2026-09-10 during SDD Task 1 of the parameter-alias-collapse plan. A subagent created
`src/tools/core/param_alias.rs` and modified two more files via codescout's MCP write tools, then
ran the gate:

```
./scripts/fmt-mine.sh
→ refused: provenance UNKNOWN for the files in question
```

```
python3 scripts/file-provenance.py src/tools/core/param_alias.rs
→ UNKNOWN   src/tools/core/param_alias.rs
            no record of any session writing this path in the window. That is a
            statement about coverage, NOT about ownership — Bash writes this tool's
            heuristics miss look identical. Do not read it as 'not mine'.
```

The refusal text is honest and its caveat is exactly right — which is what makes it hard to act
on. It cannot distinguish "nobody wrote this" from "the writer is structurally invisible".

## Reproduction

`git rev-parse HEAD` at filing: `285064ad`. Branch `experiments`.

1. Dispatch a subagent that writes a source file via `mcp__codescout__create_file` or
   `edit_code`.
2. From the controller session, run `python3 scripts/file-provenance.py <that file>`.
3. → `UNKNOWN`. Then `./scripts/fmt-mine.sh` refuses to format it.

`--all` (unbounded window) does not change the verdict, and the window was `None` anyway: the
floor is derived from the last commit touching the path (`file-provenance.py:518-524`) and the
file was untracked.

## Environment

codescout `0.15.0`, `scripts/file-provenance.py`, `scripts/fmt-mine.sh`.
Claude Code transcripts under `$CLAUDE_CONFIG_DIR/projects/<encoded-root>/*.jsonl`.
Seven Claude sessions share this checkout; `subagent-driven-development` is in active use.

## Root cause

> **Corrected 2026-09-11.** The cause recorded here until now — *"the substrate carries no
> subagent records"* — was **false**, and the correction changes what the fix has to be. The
> boundary is a **Claude Code version**, not the substrate. The original reading and how it was
> reached are kept below, because a reader who saw only the correction would repeat the inference.

**Measured 2026-09-11 across five profiles, counted by transcript file:**

| Claude Code | dispatch tool | files carrying `isSidechain: true` |
|---|---|---|
| 2.0.26 – 2.0.32 | `Task` (284 calls) | **732** |
| 2.1.220 – 2.1.268 (41 versions) | `Agent` (1,928 calls) | **0** |

No profile spans the boundary, so version and profile stay confounded in this corpus, and the
claim is stated at the strength the data supports: **across 41 distinct 2.1.x versions and 1,928
dispatches there is not one subagent record.** That is not a transient defect in one build.
`isSidechain` is written on every 2.1.x record and is never true.

**Why two counts agreed and were both wrong.** The original pair — one profile, then all profiles
— read as independent scopes. Every profile in the codescout set runs 2.1.x, so widening the
profile set never crossed the only boundary that mattered: one blind spot counted twice, which at
the point of use is indistinguishable from corroboration (`CLAUDE.md` § *Observer Blindness*,
*check independence, not agreement*).

**The confirming half, published as a denominator rather than absorbed as a catch.** This
profile/project now reads **0 of 240,108** records against the filed 0 of 88,044 — a 2.7× larger
denominator, same zero. Within 2.1.x the original measurement reproduces exactly. It was the
inference from it that failed, not the count.

**`extra.unverified` is resolved.** The parser was read against the subagent path: the attribution
loop globs `d.glob("*.jsonl")` over `transcript_roots(root)` and walks `message.content[]`
`tool_use` blocks, matching names against `CS_WRITE_TOOLS` / `NATIVE_WRITE_TOOLS` and reading the
write target from the call's path keys. There is no second location it fails to glob — a 2.1.x
session that dispatched 3 `Agent` subagents holds exactly **one** `sessionId` (its own) across
13,563 records, carrying only the parent's write calls. So the attribution is **absent rather than
mis-parsed**, and no parsing change recovers it.

**And the script documented handling for records that do not exist.** Its comment read:

> *"The record's own sessionId beats the filename: a sidechain (subagent) record carries the
> PARENT's id, which is the session a human can actually be asked about."*

A correct-sounding rule for a record shape 2.1.x never emits. The branch was unreachable, and its
presence is what made the gap invisible on a read: anyone auditing the script for subagent
coverage found a comment saying it was handled. Replaced in `ada993d6`.

**A false diagnosis this defect invites, recorded because it was made here.** The Task 1
implementer reported the cause as *"the provenance heuristic can't see writes made through
codescout's MCP tools"*, and the controller relayed it. That is false — `CS_WRITE_TOOLS` lists
`mcp__codescout__create_file`, `edit_file` and `edit_code` explicitly. The MCP tools are covered;
the *subagent* is not. Both stories predict the same observed `UNKNOWN`, and only one is true.
## Second instance, with the refusal text and a clean discriminator (2026-09-10)

An SDD fix round wrote `src/tools/core/types.rs` and `src/tools/core/tests.rs`, then ran the gate:

```
./scripts/fmt-mine.sh   -> exit 1, REFUSED
  src/tools/core/tests.rs  classified UNKNOWN
  "no record of any session writing this path in the window... 28 write(s) exist
   but predate the window; re-run with --all to see them"
```

Two things this adds to the first instance.

**The classification is `UNKNOWN`, not "owned by a peer", and the message names a WINDOW.** So the
mechanism is not "provenance cannot see MCP writes" — it explicitly can, and reports 28 historical
writes to that path. It is that the writes attributable to the session *now asking* fall outside the
window, because a subagent's writes are recorded in the subagent's own transcript file rather than
in the parent session's. The parent asks about a path it did in fact cause to be written and is
told nobody wrote it.

**Discriminator, measured in the same hour on the same checkout.** The controller session edited
`src/tools/read_file.rs` and `src/server.rs` directly through MCP `edit_file`, then ran the same
script: **exit 0**. Same tree, same window, same tool — direct writes attribute, subagent writes do
not. That isolates the cause to the transcript boundary and rules out the script, the window length
and the write tool, none of which differ between the two cases.

### The blast radius is narrower than this file first claimed — a later mechanism catches it

The refusal does NOT leave formatting unchecked, and that matters for severity. The pre-commit
sequence runs `rustfmt --check` **on the committed bytes**, and in this instance it did its job:
it refused the round's first commit attempt over a real defect (a multi-line `.expect(...)` needing
collapsing), which was fixed and the commit succeeded on retry — all without working around
`fmt-mine`'s refusal or reaching for `cargo fmt`.

So the correct path still ends in a formatted tree. What the refusal costs is not correctness but
**the error's quality and its timing**: the author learns at commit time, from a check on bytes,
rather than at gate time from a check on their own files. That is `CLAUDE.md` § *Observer
Blindness*'s third position already partly satisfied by accident — compliance leaves nothing armed
— and it is the reason this bug is an ergonomics defect with a loud fallback rather than a
correctness hole. Do not read the first instance's framing as saying formatting goes unverified;
it does not.
## Evidence

### The gate's first command is the consumer

`scripts/fmt-mine.sh:73` sets `PROVENANCE="${FMT_MINE_PROVENANCE:-$ROOT/scripts/file-provenance.py}"`
and invokes it at `:124`. So the mandatory first gate command inherits this blind spot directly.
`CLAUDE.md` § *Development Commands* states the script *"formats what `scripts/file-provenance.py`
attributes to you and **refuses** the rest"* — which, for subagent output, is everything it is
asked to format, on the rounds where anything needs formatting at all.

### The refusal blocks EVERY OTHER SESSION, not only the writer

Reported by peer sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941` (`codescout-29`, profile
`.claude-kat`) at 2026-09-10, and this file's first draft did not have it. `fmt-mine.sh` refuses on any dirty file it cannot attribute, and there is
deliberately no `--force`. So while one session's subagent holds uncommitted work, the gate's
**mandatory first command is unusable for every other session on the checkout** — the peer's run
named `src/fs/mod.rs` and `src/tools/core/tests.rs` as "needing formatting and not mine to
write", with provenance `UNKNOWN` for both.

That is a blast-radius escalation rather than a restatement of the section above. The writer at
least has the documented `cargo fmt` fallback available after checking
`git status --porcelain`; a peer has neither, because the files genuinely are not theirs and
formatting them would be the cross-session write the guard exists to prevent. **Only the writing
session can clear it, and the writing session is the one the instrument cannot see.**

Corollary the peer named and this file should carry: `run_command` attaches its `wip_authors`
block only for callers that route through it, so **a peer running the gate through native `Bash`
gets silence** — a red naming `src/fs/mod.rs` with no author, which reads as "whatever I just
committed". The peer resolved the real author positively instead, from
`CLAUDE_CONFIG_DIR` in `/proc/<pid>/environ` → the profile's `sessions/<pid>.json`, which is the
channel route `CLAUDE.md` § *Reaching a Peer Session* prescribes and which needs no cooperation
from the session being identified.



`CLAUDE.md`: *"When it refuses, it is not being unhelpful and there is no `--force`: ask the named
owner, or if you have decided it is safe, run `cargo fmt` yourself."* For an `UNKNOWN` verdict
there is no named owner to ask, so the only path is the `cargo fmt` fallback — whose blast radius
is every `.rs` in the workspace, on a checkout shared by seven sessions. **The fallback the
refusal drives you to is the wider-blast-radius action the guard exists to prevent.**

### The independence trap it sets

A session that hits the refusal naturally re-runs `file-provenance.py` to investigate and reads
its agreement as corroboration. It is not: `fmt-mine.sh` *invokes* that script, so the two are
one instrument (`CLAUDE.md` § *Reaching a Peer Session* — "check independence, not agreement").
Done here, and caught in review. The genuinely independent check is `git status --porcelain`
plus the post-`cargo fmt` diff, neither of which reads a transcript.

## Hypotheses tried

1. **Hypothesis** — the MCP write tools are missing from the selector, as the sibling defect
   `2026-09-08-the-provenance-selector-kept-the-pre-rename-tool-name` was. **Test** — read
   `file-provenance.py:58-69`. **Verdict** — rejected. All three current MCP write tools and four
   legacy names are listed.
2. **Hypothesis** — the time window excluded the writes. **Test** — re-run with `--all`.
   **Verdict** — rejected; still `UNKNOWN`, and the file was untracked so the window was `None`.
3. **Hypothesis** — subagent records exist but carry a different shape. **Test** — count
   `isSidechain` across all transcripts for this project. **Verdict** — confirmed absent: 0 true,
   88,044 false.

## Fix

**Fix 3 is shipped** — `ada993d6`, patch-id `26757097c11e0bbdc9b091a8f0f09ae960cbf902`. The dead
sidechain comment is replaced by the measurement above, and the `UNKNOWN` refusal now names
subagent writes beside Bash writes with the consequence a reader actually needs: for an
`Agent`-written file **there is no owner recorded to ask**, so stop looking for one. Verified by
rendering it against a real `UNKNOWN` path rather than read back from the source. The module
header carries the same, marked *total* rather than heuristic — the two blind spots differ in kind,
and listing them flat would misprice the subagent one.

**Fixes 1 and 2 remain open, and the correction makes them MORE owed, not less.** § Resume set an
escape condition — *"if a harness update ever begins emitting subagent records, parts 1 and 2
become unnecessary"*. The update ran the other way: 2.1.x **stopped**. The condition that would
have retired them is not merely unmet, it has moved further out of reach, so waiting on a harness
fix is not a plan.

**1 — Have the controller record its dispatches.** The controller knows the sessionId it
dispatched under and the plan/task it dispatched for. Writing a small provenance sidecar per SDD
task (git-ignored, under the plan's `.superpowers/sdd/<plan>/` workspace) gives
`file-provenance.py` a second source it can read, keyed by path. Needs no harness change.

**2 — Attribute uncommitted state from the working tree instead.** `git status --porcelain` plus
mtime cannot name a session, but it can answer the question `fmt-mine.sh` actually needs: *is any
file dirty that this session did not touch?* A narrower question than authorship, and answerable
without transcripts.

Alternatives, not stages. The choice is a design decision rather than a measurement, which is why
this file stays open with fix 3 landed rather than being closed.

Fix SHA: ada993d6 *(fix 3 only — 1 and 2 not yet fixed)*
Patch-id: 26757097c11e0bbdc9b091a8f0f09ae960cbf902 *(fix 3 only)*
## Tests added

None — not fixed. A regression test for part 3 is cheap and worth stating: assert the refusal
text names the subagent case, and assert no code path claims to read `isSidechain`. Parts 1 and 2
need a fixture with a subagent-written file, which today can only be produced by actually
dispatching one.

## Workarounds

Verify with `git status --porcelain` that no other session's files of the same language are
dirty, then run `cargo fmt` directly — the documented fallback. Do **not** cite a
`file-provenance.py` re-run as a second opinion; it is the same instrument.

## Resume

Fix 3 is done and the root cause is corrected. **Do not re-run the `isSidechain` count as this
section previously instructed** — it has been run at a wider scope than that instruction intended
(five profiles, grouped by version) and the answer is in § Root cause. Re-running it per-profile
returns the same zero for the same reason, and reads as confirmation of the claim it actually
falsified.

What is left is one decision: **fix 1 (controller sidecar) or fix 2 (working-tree question)**.
Fix 2 is the smaller claim — it answers `fmt-mine.sh`'s real question (*is anything dirty that I
did not touch?*) without attributing authorship at all — and is worth pricing first for exactly
that reason, since authorship is the part the substrate cannot give back.

**Re-check one thing before building either:** whether 2.1.x records subagent activity anywhere
*outside* `$CLAUDE_CONFIG_DIR/projects/*/*.jsonl`. What was checked is that no second `sessionId`
appears inside the parent's own file; what was **not** checked is whether the harness introduced a
separate directory alongside the `Task` → `Agent` move. If such a location exists, both fixes
collapse to a glob change.

**This is the third provenance defect on a version or rename boundary** — the other two are in
`extra.related`. All three sit in provenance/gate tooling, so the class does not yet meet the
≥ 2-subsystem half of the promotion bar. Named here so the fourth is recognised rather than
re-diagnosed.
## References

- `scripts/file-provenance.py:58-69` — the selector, which is correct.
- `scripts/file-provenance.py:450-452` — the unreachable sidechain comment.
- `scripts/fmt-mine.sh:73,124` — the consumer, and the gate's first command.
- `CLAUDE.md` § *Development Commands* — the "refuses the rest" contract and the no-`--force` rule.
- `docs/issues/archive/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md` and
  `docs/issues/archive/2026-09-08-the-provenance-selector-kept-the-pre-rename-tool-name.md` —
  both `fixed`, both this instrument, both a *selector* defect. This one is a substrate defect
  and no selector change reaches it.

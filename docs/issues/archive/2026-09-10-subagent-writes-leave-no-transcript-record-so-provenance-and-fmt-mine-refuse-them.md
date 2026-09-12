---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
- provenance
- sdd
- fmt-gate
claimed_at: 2026-09-12
claimed_by: 05841db2-4ba0-4cb2-a22f-c0bc2f771e20
closed: 2026-09-12
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

> **Corrected again 2026-09-12, and this is the third reading of one cause.** The version
> boundary below is **false**. Claude Code 2.1.x does record subagent activity — it writes it to
> `<project-dir>/<parent-session-id>/subagents/agent-<id>.jsonl`, one directory below the
> parent's own transcript. `scan()` globbed `d.glob("*.jsonl")` **non-recursively**, so every one
> of those files lay outside the window, and every count of "subagent records" was taken through
> that same glob. **A windowed instrument's zero is scoped to its window.** The two readings below
> are kept because the SHAPE is the finding: each correction narrowed the blame — substrate, then
> version — and neither questioned the instrument, because re-deriving through it reproduced the
> zero on demand and read as corroboration.
>
> Re-derived 2026-09-12 **outside** the glob, this checkout, three profiles: **756** subagent
> transcript files, **755** carrying `isSidechain: true`, **192,797** such records. The newest was
> written that same day at 08:37 by Claude Code **2.1.267** — inside the exact `2.1.220–2.1.268`
> range the table below cites as emitting none. Each record carries `isSidechain: true`, a
> timestamp, an `agentId`, and a `sessionId` holding the **parent's** id.
>
> **§ Resume called this exactly.** It asked whether 2.1.x records subagent activity anywhere
> outside `projects/*/*.jsonl`, said that had **not** been checked, and predicted that *"if such a
> location exists, both fixes collapse to a glob change"*. It does, and they do. Credit to the
> author of that section (sessionId `ec98641c-d5ba-456d-8e4a-10e24d52da16`), who named the open
> question and its consequence without having the answer — the superseded § Root cause sentence
> *"There is no second location it fails to glob"* is the claim that reads as settled and was not.

**Superseded readings, kept for their derivations — do not act on the table below.**

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

**Fixed 2026-09-12 by a glob change, exactly as § Resume predicted.** Fixes 1 (controller
sidecar) and 2 (working-tree question) are **not needed and were not built** — both existed to
work around a substrate gap that is not there.

`scan()` now walks `<project-dir>/*/subagents/*.jsonl` beside `<project-dir>/*.jsonl`. **No
parsing change was required**, which is the sharpest refutation of the superseded comment's *"no
parsing change recovers them"*: the loop body was already correct for these records, since
`who = rec.get("sessionId") or …` picks up the parent id that subagent records carry.

One accompanying correction, which is a real defect and not cosmetic: the fallback
`sid = f.stem` yields `agent-<id>` for a subagent file, and that addresses **no session and no
human**. It now derives the parent session id from the directory instead. A refusal naming
`agent-a0123…` as the owner would be *worse* than `UNKNOWN` — `UNKNOWN` says "cannot tell", that
would say "go ask a session that does not exist".

**Three prose surfaces carried the falsified claim and all three are corrected**, because the
reader acts on the message, not the code: the module header, the in-loop comment, and — the
expensive one — the user-facing `UNKNOWN` text, which read *"If this path came out of a subagent
task, **stop looking for the owner** — there is none recorded"*. The tool was telling its reader
to stop looking, one directory above the answer.

**Verified on real data, not only fixtures.** A/B of HEAD's scanner against the fix on files a
real subagent wrote this morning:

| path | writes seen BEFORE | AFTER |
|---|---|---|
| `src/tools/markdown/edit_markdown.rs` | 43 | **61** |
| `src/tools/symbol/symbols.rs` | 66 | **103** |

And the verdict itself changes in the way that matters. `--all` on `edit_markdown.rs` before:
**10** owners named, **0 of 10 live** — every one `[not live — cannot be asked]`. After: **11**
owners, the new one `b0b9bc40-…` marked `[LIVE]`, resolved to `codescout-75`, pid, profile, cwd
and a `uds:` socket. `fmt-mine.sh`'s entire remedy is *ask the named owner*; on subagent-touched
files it had no reachable party to name, and now it does.

Fix SHA: `03904fe4`
Patch-id: `eea6a8ca06de7593b1c88faf0c5ddbcc4790271a`

**Supersedes the partial anchor.** `ada993d6` (fix 3) corrected the prose to state the
version-boundary reading, which this fix falsifies; that prose is rewritten here. The pair above
is the whole fix, not a stage.
## Tests added

Seven assertions in `tests/file-provenance.sh`, run by CI's `shell-tests` job. Four were
observed **red** before the fix; the other three passed against the unfixed code, which is what
makes them controls rather than duplicates.

- **The regression.** A subagent record under `<parent-sid>/subagents/` attributes to the parent
  (`MINE`), and the verdict is no longer `UNKNOWN`.
- **Control — credit.** A *peer's* subagent record attributes to the `PEER` and never to this
  session. Without it, "glob recursively and take the credit" passes the regression above and
  hands this session write authority over a peer's files, which is precisely what `fmt-mine.sh`
  then acts on.
- **Control — the fallback.** A subagent record carrying no `sessionId` falls back to the parent
  DIRECTORY, never to the `agent-<id>` filename.
- **The message.** Three assertions on the `UNKNOWN` text: it still names Bash as the real blind
  spot, no longer says *"stop looking for the owner"*, and no longer repeats the falsified
  zero-records claim. Deliberately a **shape** test, not a prose pin — it reds on the sentence
  coming back and survives rewording. This file's own § Tests added proposed exactly such an
  assertion on 2026-09-11 and it was never written, which is how the message went on telling
  readers to stop looking for a month.

The fixture mirrors a real 2.1.x record field for field, as measured on disk, with an inline note
saying so — a fixture invented from the description would have been the fourth reading of this
cause taken through a source that could not falsify it.

**Verified beyond the suite:** A/B against HEAD's scanner on files a real subagent wrote that
morning (see § Fix), plus the live gate — `./scripts/fmt-mine.sh` refused correctly and named a
reachable live peer.
## Workarounds

Verify with `git status --porcelain` that no other session's files of the same language are
dirty, then run `cargo fmt` directly — the documented fallback. Do **not** cite a
`file-provenance.py` re-run as a second opinion; it is the same instrument.

## Resume

Fixed and archived. Fixes 1 and 2 are **withdrawn, not deferred** — each was a workaround for a
substrate gap that does not exist, and building either would have added a second source of truth
for data the harness already writes.

The standing note about the class is kept and **updated**: this is the third provenance defect on
a version or rename boundary, and it is the one where the boundary turned out to be a **reading**
rather than a boundary. The other two are in `extra.related`. All three still sit in
provenance/gate tooling, so the class does not meet the ≥ 2-subsystem half of the promotion bar.

**What the fourth instance should check first**, phrased as an action rather than a lesson: when a
count of "does the substrate contain X" returns zero, re-derive it with an instrument that does
not share the first one's window — here, `find` instead of the module's own glob. Three readings
agreed here because all three were the same reading.
## References

- `scripts/file-provenance.py:58-69` — the selector, which is correct.
- `scripts/file-provenance.py:450-452` — the unreachable sidechain comment.
- `scripts/fmt-mine.sh:73,124` — the consumer, and the gate's first command.
- `CLAUDE.md` § *Development Commands* — the "refuses the rest" contract and the no-`--force` rule.
- `docs/issues/archive/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md` and
  `docs/issues/archive/2026-09-08-the-provenance-selector-kept-the-pre-rename-tool-name.md` —
  both `fixed`, both this instrument, both a *selector* defect. This one is a substrate defect
  and no selector change reaches it.

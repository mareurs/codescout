---
kind: bug
status: open
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

**The substrate carries no subagent records.** Measured 2026-09-10 over this profile's
transcripts for this project:

```
transcript lines:  111,968
"isSidechain":true      0
"isSidechain":false   88,044
```

The field is present on 88,044 records and **never true**. An independent count over all
profiles (579,518 lines) by a second reader returned the same zero — two different scopes,
same result, so this is the substrate rather than one profile's quirk.

`file-provenance.py` attributes by walking `message.content[].tool_use` blocks and matching the
tool name against `CS_WRITE_TOOLS` / `NATIVE_WRITE_TOOLS` (`:58-69`), reading the write target
from the call's path keys (`:384-387`). With no subagent records in the file, that loop never
sees a subagent's `create_file` — so the attribution is not wrong, it is **absent**.

**And the script documents handling for the records that do not exist.** `:450-452`:

> *"The record's own sessionId beats the filename: a sidechain (subagent) record carries the
> PARENT's id, which is the session a human can actually be asked about."*

That is a correct-sounding rule for a record shape the transcripts never contain. The branch is
unreachable, and its presence is what makes the gap invisible on a read: anyone auditing the
script for subagent coverage finds a comment saying it is handled.

**A false diagnosis this defect invites, recorded because it was made here.** The Task 1
implementer reported the cause as *"the provenance heuristic can't see writes made through
codescout's MCP tools"*, and the controller relayed it. That is false — `:58-65` lists
`mcp__codescout__create_file`, `edit_file` and `edit_code` explicitly. The MCP tools are covered;
the *subagent* is not. Both stories predict the same observed `UNKNOWN`, and only one is true.

Measured 2026-09-10 by the counts above; predicate read at `285064ad`.

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

Not implemented. The attribution cannot be recovered from a substrate that holds no record, so
every option below changes *what is read* rather than how it is parsed.

**1 — Have the controller record its dispatches.** The controller knows the sessionId it
dispatched under and the plan/task it dispatched for. Writing a small provenance sidecar per SDD
task (git-ignored, under the plan's `.superpowers/sdd/<plan>/` workspace) gives
`file-provenance.py` a second source it can read, keyed by path. Cheapest of the three and needs
no harness change.

**2 — Attribute uncommitted state from the working tree instead.** `git status --porcelain` plus
mtime cannot name a session, but it can answer the question `fmt-mine.sh` actually needs: *is any
file dirty that this session did not touch?* That is a narrower question than authorship and is
answerable without transcripts.

**3 — Delete the unreachable sidechain branch and say so at the refusal site.** Independent of 1
and 2, and owed regardless: the comment at `:450-452` asserts coverage that does not exist, and
the refusal text should name subagent-written files as a known blind spot so the reader stops
looking for an owner who was never recorded.

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

Decide between fix 1 (controller sidecar) and fix 2 (working-tree question) — they are
alternatives, not stages. Ship fix 3 either way. Before starting, re-run the `isSidechain` count:
if a harness update ever begins emitting subagent records, parts 1 and 2 become unnecessary and
only the stale comment needs removing.

## References

- `scripts/file-provenance.py:58-69` — the selector, which is correct.
- `scripts/file-provenance.py:450-452` — the unreachable sidechain comment.
- `scripts/fmt-mine.sh:73,124` — the consumer, and the gate's first command.
- `CLAUDE.md` § *Development Commands* — the "refuses the rest" contract and the no-`--force` rule.
- `docs/issues/archive/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md` and
  `docs/issues/archive/2026-09-08-the-provenance-selector-kept-the-pre-rename-tool-name.md` —
  both `fixed`, both this instrument, both a *selector* defect. This one is a substrate defect
  and no selector change reaches it.

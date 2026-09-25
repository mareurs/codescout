---
status: open
opened: 2026-09-13
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/gate-keyed-on-unobservable-event
kind: bug
---

# The worktree guard's `cd`-chain remedy is stripped before the guard can see it

The `git-worktree-guard.mjs` hook refuses a worktree-ambiguous git mutation and
prints **two** escapes. One of them cannot be followed on this harness, and the
refusal that prints it is the same refusal you get for not following it.

```
Fix one of:
  • Use explicit path:      git -C /full/worktree/path commit ...
  • Chain cd in same call:  cd /full/worktree/path && git commit ...
```

The second form is stripped out of the command string **before the hook runs**,
so the hook judges a bare `git push` and refuses again.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `8e274b32`) — not reproducible here: dormant, not fixed.**
- **Unreachable today.** Native `Bash` is in `permissions.deny` on all three profiles (`~/.claude`, `~/.claude-sdd`,
  `~/.claude-kat`), and the guard matches `Bash` only (`git-worktree-guard.mjs:87`, `claude-plugins` repo). So
  neither the harness strip nor this refusal can be reached.
- **Nothing changed.** The remedy line is unchanged since `claude-plugins:15fa367`.
- **The guard's own half still holds.** The guard was run on synthetic PreToolUse JSON:
  - `cd <checkout> && git push --dry-run …` is exempt.
  - A bare `git push --dry-run …` is denied.
  - The exemption is text-only: `cd <nonexistent path> && git commit` is exempted too.
  The strip itself stays untested.
- **The discriminating test's target is gone.** `codescout.worktrees/check-codescout-integration` no longer exists;
  only `mutation-slot-0` remains. The test stays unrun.
- **Unexplained, and not investigated: the 2026-09-22 recurrence below came through `Bash`,** after the 2026-09-20
  deny date `CLAUDE.md` gives. When the deny actually took effect was not established.
- **If `Bash` is re-allowed,** re-probe the strip before trusting the remedy text.

## Reproduction

Three calls from a `Bash` tool whose CC PWD is `/home/marius/work/claude/codescout`
(a repo with 2 worktrees, so the guard's carve-out does not apply):

| # | command typed | result |
|---|---|---|
| 1 | `cd /home/…/codescout && CODESCOUT_PUSH_ACK="…" git push origin experiments 2>&1; echo …` | BLOCKED |
| 2 | `cd /home/…/codescout && git push --dry-run origin experiments 2>&1; echo …` | BLOCKED |
| 3 | `cd /home/…/codescout && git push --dry-run origin experiments` | BLOCKED |

Call 3 is the documented form **verbatim** — no env-var prefix, no `;`, no
redirection. It is refused.

### Recurrence 2026-09-22 — same strip, different verb

Reproduced on `git commit`, which widens the observed surface: all three calls in the table
above are `git push`, so the verb was a shared feature of every prior instance and could not be
ruled out as part of the mechanism. It is not.

Typed: `cd /home/marius/work/claude/codescout && git commit -F - <<'MSG' … MSG`
Echoed back: `Offender: git commit -F - <<Q` — the `cd … &&` absent again, exactly as the
`Command:` analysis above predicts. `git -C /home/marius/work/claude/codescout commit -F -`
then succeeded on the first attempt (`ce9aa15f`).

Two details worth adding to the cost section rather than the mechanism:

- **The worktree count in the refusal has grown to 15**, from the 2 this record was written
  against — 13 of them `mutation-<uuid>` trees from `scripts/mutation-probe.sh`. The guard's
  single-worktree carve-out is therefore further from applying than ever here, so this refusal
  is now effectively unconditional on this checkout rather than incidental.
- **The refusal is still the cheapest place to learn the repo's worktree inventory**, which is
  how this session noticed HEAD had moved three commits under it mid-task. That is an accident
  of the error text, not a feature, but it is the reason the refusal cost ~nothing this time.

**The discriminating test in § *What is NOT established* remains UNRUN, and deliberately.** It
asks for a guarded verb with `cd <other-worktree> &&`, and on inspection every guarded verb
available for it is unsafe to fire speculatively from this session: `push` contacts the remote,
and `commit` / `reset` / `rebase` / `merge` would act on a worktree whose index this session does
not own — on a checkout that currently has four other live sessions. The test is right and still
wants running; it wants running by someone who can first establish that the target worktree's
index is empty. Recording the reason so the next reader does not read the gap as an oversight.

## The evidence is in the guard's own output

The refusal echoes what it received. For call 3:

```
Command : git push --dry-run origin experiments
Offender: git push --dry-run origin experiments
```

The `cd /home/…/codescout && ` I typed is absent from `Command:`, which is the
verbatim input string (`input.tool_input.command`). So the hook never saw it.

That last call also ended with a harness line the earlier ones did not surface:

```
Shell cwd was reset to /home/marius/work/claude/codescout
```

## Mechanism

**The guard's logic is correct and its tests would pass.** Reading
`claude-plugins/codescout-companion/hooks/git-worktree-guard.mjs`:

- `segments()` splits on `(\|\||&&|;|\||\n)` — so `cd X && git push` is two segments.
- `CD_TO_PATH = /^\s*cd\s+\S+\s*$/` matches a segment that is exactly `cd <path>`.
- A matching `cd` segment exempts **every later segment**, with a comment saying
  this is what keeps "the documented `cd /p && git commit && git push`" working.

Every one of those behaves as written. The defect is that the input is rewritten
one layer up: the harness lifts a leading `cd <path> &&` into a real working-directory
change and removes it from the string the hook is handed. The guard's exemption is
keyed on a token that no longer exists by the time it looks — while the condition the
token stood for (the shell really is in that directory) is *true*.

This is `IC-2`: the gate proxies "will this git run in the right worktree?" with
"does the command text contain a `cd`?", and the proxy is unobservable here.

## What is NOT established

- **Whether the strip is conditional on the `cd` target equalling CC PWD.** Every
  call above cd'd to the *same* directory CC was already in, which is exactly when
  such a strip would be a no-op optimisation. A `cd` to the *other* worktree may well
  survive, which would make the remedy work precisely when it is needed and fail only
  when it is redundant. **Discriminating test:** run a guarded verb with
  `cd /home/marius/work/claude/codescout.worktrees/check-codescout-integration && …`
  and read the `Command:` echo — if the `cd` is present there, the strip is
  PWD-conditional and the remedy text needs a caveat rather than a deletion.
- **Which layer performs the strip.** Inferred from the `Command:` echo plus the
  `Shell cwd was reset` line; the rewriting code was not read.
- Whether `Bash`'s documented "`cd` in a compound command can trigger a permission
  prompt" note is the same mechanism.

## Cost

Low per occurrence, unbounded in aggregate: `git -C <path>` works and is listed
first. The cost is that a reader who picks the second escape gets the identical
refusal with no signal that the escape itself is the problem — and the refusal
re-offers it. I spent three calls and one wrong diagnosis (I blamed the
`CODESCOUT_PUSH_ACK=` env prefix; the control in call 2 falsified that) before
reading the `Command:` field that had the answer in it from the first refusal.

## Prior art

Four worktree-guard bugs are archived, all fixed, none of them this:

- `docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md`
- `docs/issues/archive/2026-09-03-worktree-guard-word-boundary-blocks-read-only-git-plumbing.md`
- `docs/issues/archive/2026-09-02-a-git-verb-regex-swallows-longer-subcommands-sharing-its-prefix.md`
- `docs/issues/archive/2026-09-01-staging-op-reads-a-detached-flag-value-as-the-subcommand.md`

All four are defects *inside* the matcher. This one is the matcher being right
about an input it was not given.

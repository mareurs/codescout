---
id: d7bbeba8a9f23dd8
kind: bug
status: open
title: 'BUG: an absent session-stage-log makes the foreign-index guard pass silently — every producer failure path exits 0'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
closed: ''
opened: 2026-09-01
owner: marius
related: []
severity: high
unverified: 'The fail-open is MEASURED (EXIT=0 with a peer''s file staged). What is NOT established is a production trigger: the log was removed by hand in the repro, and no observed production run has been shown to lose the log. The one candidate — a 3-case suite failure on this session''s first run — is n=1 and did not reproduce in 12+ attempts.'
---

## Summary
`scripts/pre-commit-foreign-index.sh` decides whether to refuse by reading
`.git/session-stage-log`. When that file is **absent**, the guard exits 0 — it passes a commit
carrying a peer's staged paths, silently, with no output. Every error path in the producing
hook (`scripts/post-index-change-stage-log.sh`) is `exit 0`, so any transient that stops the
log being written disarms the guard for exactly as long as the log stays missing.

This is the same end state as the production incident the guard was written for
(`docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md`):
there the log was *wrong*, here it is *gone*, and both make the guard pass.

## Symptom (Effect)
One temp repo, one staged foreign file, two guard runs — the only variable is whether the log
file exists:

```
--- log after session A stages peerfile.txt ---
-	c9f6d41	peerfile.txt

--- guard as session B, log PRESENT ---
  (refusal message)                                        EXIT=1

--- guard as session B, log ABSENT (rm -f, no other change) ---
                                                           EXIT=0

--- still staged, would ride into B's commit ---
A	peerfile.txt
```

`EXIT=0` with `peerfile.txt` staged is the capture the guard exists to prevent, and it emits
nothing at all.


## Escalation — a lost log does not merely fail open, it CROSS-CLAIMS

**Measured 2026-09-01, after this file was first written.** The framing above ("absent log →
guard passes") understates it. The log is rebuilt from `git diff --cached --raw` on every index
write, and `:135` is `[ -n "$owner" ] || owner="$claimant"` — so a pair with no surviving row
is assigned to **whoever caused the current write**, not to `-`.

Consequence: with the log lost, one session staging **one** file becomes the recorded owner of
**every** currently-staged path, including its peers'. Session A had staged `f.txt` and `g.txt`;
the log was removed; session B then staged only `h.txt`:

```
bbbb-B   f.txt     <- A's work, now recorded as B's
bbbb-B   g.txt     <- A's work, now recorded as B's
bbbb-B   h.txt     <- the only file B staged
```

The foreign-index guard then sees nothing foreign and passes **silently** — which is the
production capture at `3a422b31` that started this whole line of work, reached by a second road.

**This inverts the direction, and that is why it matters more than the original framing.**
`a987df96`'s ruling is *unknown over-refuses recoverably, where `mine` under-refuses silently —
prefer the noisy wrong answer where the quiet one is unobservable.* Both `e3c75306` (seed at
install) and `a987df96` (non-staging writes record `-`) patch specific entry points toward that
rule, but the **fallback itself still defaults to `claimant`**, so the cold-log path lands on
exactly the direction the ruling forbids.

**Second measured property — `-` is sticky, and it explains a peer's live symptom.** The lookup
at `:131-134` keys on `(blob, path)`, so once a pair is stamped `-` the true owner re-staging
*other* files never recovers it. Verified: A staged `f.txt` (owner A); log removed; a peer
`git status` stamped `f.txt` as `-`; A then staged `g.txt` — `g.txt` got `A` and `f.txt` stayed
`-`. It recovers only when the file's **content** changes, minting a new blob and therefore a
new key. Peer `codescout-68` reported `docs/trackers/bug-fix-session-log.md` sitting at `-`
despite a bare `git add` with a live session id, and hypothesised a `/proc/$PPID` divergence
under Claude Code's Bash tool. **That hypothesis is rejected** — a bare `git add` after a plain
`cd` in a Bash tool call attributes correctly, measured. Stickiness explains it without it.

**Why the fix is not obvious, and why it is not attempted here.** "Record `-` when the log is
cold" is wrong as stated: a genuinely new pair created by the current staging op *should* be
claimed by the stager, and from the rebuild alone the hook cannot distinguish "pair I just
created" from "pair whose row was lost". Distinguishing them needs a signal the rebuild does
not carry — the pathspec from argv, or a durable marker that the log was once non-empty. That
is a design decision with a real failure mode either way, so it is left for a ruling rather
than guessed at.

## Measurements 2026-09-01 — two of which correct this file

Run before choosing a fix, because the fix is a claim about a population. All in throwaway repos
under `$TMPDIR` against the real hook; the shared checkout was never written to.

**M1 — the cold state is ROUTINE, not exotic. This file said "lost"; that understates it.**
The log is rebuilt from `git diff --cached --raw` on every index write, so an empty index writes
a **0-byte** log and `[ -s "$log" ]` is then false. Reached with no deletion at all: commit, then
any index write. `git commit` *does* fire the hook (instrumented, 1 firing), but the log empties
on the **next** write, not during the commit.

**M2 — and the routine cold state is HARMLESS. This is the correction that matters.**
Log-empty ⇔ index-empty, so when the log is cold there is by construction nothing staged to
mis-attribute. Each staging op warms the log with its own rows before any peer's write lands:

```
AAAA   a.txt
BBBB   b.txt      <- cold log, two sessions, still correct
```

So the danger is **not** "the log went cold". It is "the log was cold *while content was staged*",
which the routine path cannot produce.

**M3 — the real trigger is ONE missed hook run, and no deletion is needed.**
A staged `peer.txt` with its hook suppressed (one inherited `CODESCOUT_STAGE_LOG_RUNNING`), then
B staged `mine.txt` normally:

```
BBBB   mine.txt
BBBB   peer.txt   <- A's file, now recorded as B's
B's bare commit would carry: mine.txt, peer.txt
```

Any of the four `exit 0` paths reaches this. The original repro's `rm -f` was a stand-in for a
mechanism that is ordinary.

**M4 — pathspec coverage is 73.8%, so a pathspec discriminator is not an empty population.**
Across 573 session transcripts and 107,488 Bash invocations in this project, 1,585 real
`git add` calls (extracted from the `"command"` field only — a first pass over transcript prose
inflated this to 3,406 by counting markdown backticks as invocations):

| bucket | n | share |
|---|---|---|
| explicit pathspec | 1,170 | **73.8%** |
| blanket (`-A` 383, `-u` 8) | 391 | 24.7% |
| other flag-led | 24 | 1.5% |

Buckets are mutually exclusive and sum to 1,585. A looser first pass reported "64 bare
`git add .`"; there are **zero** — those were `git add .codescout/…` and `.claude/…`, explicit
paths beginning with a dot.

### The ruling this sets up

| policy | peer's staged file | your own file | cost |
|---|---|---|---|
| **P0** current — `owner = claimant` | **claimed as yours**, silently | correct | under-refuses; violates `a987df96`'s own rule |
| **P1** `-` whenever the log is cold | safe `-` | **`-` too** | refuses your own routine commits — the version that gets the guard switched off |
| **P2** claimant only for argv-named paths, `-` otherwise | safe `-` | correct for 73.8% | over-refuses on the 24.7%, in the recoverable direction |

P2's uncovered remainder is `git add -A` / `-u`, which `scripts/pre-commit-foreign-index.sh`'s
own header **already declares uncoverable**: *"it cannot cover a `git add -A` under your own id,
where every path reads as yours by construction."* So P2 shrinks an existing blind spot rather
than creating a new one.

**Not yet ruled on** — P2 is a policy change to a guard every session in this checkout shares,
so it is left for a deliberate decision rather than taken as the obvious reading of a
measurement.
## Reproduction
Verified 2026-09-01 in a throwaway repo under `$TMPDIR`; the real checkout was never touched.

1. Temp repo, `.git/hooks/post-index-change` wired to `post-index-change-stage-log.sh`.
2. `CLAUDE_CODE_SESSION_ID=AAAA git -C "$P" add peerfile.txt`
3. `cd "$P" && CLAUDE_CODE_SESSION_ID=BBBB bash scripts/pre-commit-foreign-index.sh` → `EXIT=1` (correct).
4. `rm -f "$P/.git/session-stage-log"`, repeat step 3 → `EXIT=0`, no output, file still staged.

## Environment
`scripts/pre-commit-foreign-index.sh` + `scripts/post-index-change-stage-log.sh` at `a987df96`.
Shared checkout `/home/marius/work/claude/codescout`, branch `experiments`, Linux.

## Root cause
The guard treats "no log" as "nothing foreign" — an absent proxy is read as a negative result
rather than as *no answer*. `IC-2` (`cluster/gate-keyed-on-unobservable-event`) verbatim: the
guard cannot observe "did a peer stage this?", substitutes a file it does not own, and the
substitute fails silently.

The producer makes that state easy to reach. `post-index-change-stage-log.sh` returns `exit 0`
on every failure:

- `:83` re-entrancy guard — an inherited `CODESCOUT_STAGE_LOG_RUNNING` disables it outright
- `:87-88` `git rev-parse --git-dir` failure
- `:124` `: > "$tmp"` failure
- `:141` `mv -f "$tmp" "$log" 2>/dev/null || rm -f "$tmp"` — a failed rename **deletes** the
  new log and leaves the old one stale or absent

None of these is observable by anyone. The log is rebuilt from scratch on every index write and
atomically renamed into place, so a single failed write leaves no trace and the next guard run
reads whatever survived.

**Measured corollary — the re-entrancy path alone disarms it completely.** Running
`tests/hooks-discrimination.sh` with `CODESCOUT_STAGE_LOG_RUNNING=1` inherited fails 11 of 21
cases — and **10 still pass**, among them `own paths only -> silent`, `pathspec commit ->
silent`, `nothing staged -> silent` and `no session id -> silent`. Those are absence
assertions, monotone under removal (CLAUDE.md § *Testing Discipline*, first law): they are
satisfied exactly as well by a guard that has been switched off.

## Evidence
The `EXIT=1` / `EXIT=0` pair above, same repo, same staged content, same session ids, one
`rm -f` between them.

## Hypotheses tried
1. **The suite already covers this.** REJECTED, read at the source. § *stager wins* covers
   `rm -f log` **followed by a peer `git status`**, which re-fires the hook and rebuilds the log
   with `-` rows; the guard then refuses on those. The assertion chain is
   `cold log + peer status -> unknown` → `unknown reads as foreign -> refuse`. No case runs the
   guard while the log is *still* missing.
2. **`git status` might not fire `post-index-change`, making the gap unreachable.** REJECTED,
   measured: instrumented hook fired 5/5 on consecutive `git status --short`, and again after
   `touch`. The hook is reliable; the gap is reached by the producer failing, not by git.

## Related observation — one unreproduced flake
The first run of the suite this session failed 3 cases with
`awk: cannot open .git/session-stage-log`, i.e. the log was absent where § *stager wins*
expects it rebuilt. It has not reproduced in 12+ subsequent runs (fresh copies and in-place),
and the run happened while five sessions were active on this machine. Recorded as an
observation, **not** as a claim: candidate mechanism is one of the silent `exit 0` paths above
firing transiently, which is exactly what this bug describes, but n=1 and unconfirmed.

## Fix
Not attempted. The shape follows `a987df96`'s own ruling — *unknown over-refuses recoverably;
prefer the noisy wrong answer where the quiet one is unobservable* — which the producer now
honours per-pair (`-`) but the consumer does not honour for the log as a whole:

- guard: distinguish **"log absent"** from **"log lists nothing foreign"**, and refuse (or at
  minimum warn loudly) on the former, since staged paths with no log is unattributable, not safe;
- producer: make the failure paths emit something a reader can act on rather than `exit 0`.

Note the guard cannot simply refuse whenever the log is missing without a bootstrap story —
`e3c75306` seeds the log at install precisely because a cold log is legitimate at that moment.
"Staged paths exist AND no log" is the discriminating condition.

## Tests added
None yet. Owed: a case that runs the guard with staged foreign content and **no** log present,
asserting a refusal — and a re-entrancy case asserting the suite fails loudly rather than
10-of-21 quietly when the hook is disabled.

## Workarounds
The composition already documented in the hook header: `git add <paths>` then
`git commit -- <same paths>`. The pathspec form ignores the shared index, so it does not depend
on the log being correct or present.

## Resume
Open. Mechanism measured and reproduced; fix and regression cases owed.

## References
- `scripts/pre-commit-foreign-index.sh` (consumer)
- `scripts/post-index-change-stage-log.sh:83,87,124,141` (the silent `exit 0` paths) — line numbers verified against `71499331`
- `docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md` — same end state, different cause
- `docs/issues/archive/2026-09-01-staging-op-reads-a-detached-flag-value-as-the-subcommand.md` — found in the same pass; fixed at `7278508e` and archived

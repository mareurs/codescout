---
kind: bug
status: fixed
title: peer-sessions.sh collects process start time but never compares it to the binary's build time, so a peer serving stale bytes reads as healthy
tags:
- cluster/blast-radius-exceeds-visibility
closed: 2026-09-01
opened: 2026-08-31
owner: marius
severity: med
---

## Summary

`scripts/peer-sessions.sh` prints each peer's `lstart`, `cwd` and `comm`. It never reads
`/proc/$pid/exe` and never compares start time against the served binary's mtime — so the
one fact that decides whether a peer's measurements are current is absent from the report
that exists to characterise peers.

Measured on this host at 2026-08-31T21:47, immediately after a rebuild: **9 of 13 live
`codescout start` processes were serving pre-rebuild bytes**, two of them ~85 hours and
~420 commits behind.

## Symptom (Effect)

Binary mtime `2026-08-31T21:26:20`. Every process below resolves `/proc/$pid/exe` to that
same path, so path identity discriminates nothing:

```text
pid      | started      | vs build | exe
225825   | 08-28 08:17  | STALE    | codescout      <- ~85h, ~420 commits behind
851714   | 08-28 09:23  | STALE    | codescout
1888275  | 08-31 11:20  | STALE    | codescout
1998675  | 08-31 11:38  | STALE    | codescout
2122896  | 08-31 12:04  | STALE    | codescout
2900418  | 08-31 14:32  | STALE    | codescout
3001659  | 08-31 14:46  | STALE    | codescout
4008514  | 08-31 17:27  | STALE    | codescout
821425   | 08-31 21:27  | fresh    | codescout
823396   | 08-31 21:27  | fresh    | codescout
984066   | 08-31 21:42  | fresh    | codescout
997544   | 08-31 21:45  | fresh    | codescout
```

**The consequence is a peer that reproduces an already-fixed defect and reports it as
current.** Concretely, against the two 08-28 processes: `03618605` (2026-08-28 07:50) made
`cross_repo_file_qualified` reachable. Process 225825 started 08-28 08:17 — 27 minutes
after that commit, but from a binary built before it. A session on that server measuring
`link_scan` today gets `cross_repo_file_qualified: 0` and 20 in `malformed_qualifier`, which
is exactly the evidence table in
`docs/issues/archive/2026-08-27-cross-repo-file-qualified-bucket-never-fires.md`. It would
re-file a fixed bug, with correct numbers, from a stale world.

## Root cause

**`~/.cargo/bin/codescout` is a symlink** into the working tree —
`-> /home/marius/work/claude/codescout/target/release/codescout`, created 2026-06-02 — and
`sha256sum` confirms one set of bytes, not two. That is a deliberate simplification: it
collapses `R-89`'s distribution axis, so there is no install step to forget.

The side effect is that it also collapses the only *cheap* staleness signal. With a copied
install you can diff two files; with a symlink every process has the correct path, the
correct hash, and a different build. **Process start time versus binary mtime is the only
remaining discriminator, and it is the one comparison the script does not make.**

`peer-sessions.sh:116` already collects the left operand (`ps -o lstart=`). The right operand
is never collected.

## Evidence

Verified with two independent methods that agree: `ps -eo pid,lstart` and
`stat -c %Y /proc/$pid`, matching to the minute on every pid (e.g. 225825 → `Aug 28
08:17:23` and `08-28 08:17`).

`/proc/$pid/exe` is readable for every peer here — same uid — so the proposed fix is not
blocked by permissions. Checked, not assumed.

For the session that ran this scout, all three `R-89` axes are green and were verified
rather than inferred: build mtime `21:26:20`; serving pid 997544 started `21:45:06`, which
is *after* the build; `/proc/997544/exe` resolves to that exact file; and the symlink's
hash equals the built file's. That is the shape of a positive result, and it is why this
bug's population claim can be trusted — the same probe that cleared this process indicted
nine others.

## Fix

**Fixed 2026-09-01 on `experiments`, by the corrected form below — not the one first proposed here.**

| | SHA | patch-id |
|---|---|---|
| the fix | `4816d64f` | `efe2bd3aa69a50734a0e12a25bddea388d709cf4` |
| the regression test + its runner | `ce39b8f4` | `825cf81d56845f82d259fb4e64c0d65cfc176399` |

`binary_state()` in `scripts/peer-sessions.sh` reads the ` (deleted)` suffix directly, exactly as
§ *The proposed fix above FAILS OPEN* prescribes. The report prints `cs REPLACED` per row and
counts them in the closing summary, carrying the "a peer's numbers are evidence about the build it
LOADED" note this file asked for, beside the existing authorship warning.

**Verified by two independent instruments rather than by reading the code.** Unfiltered,
`peer-sessions.sh` names sessions `3624594`, `3632455`, `3639628` as `REPLACED`. Those are exactly
the parent sessions of the three stale codescout servers a separate `ps`-start-time-vs-binary-mtime
scan found among nine. Nine processes, both outcomes present, zero disagreements — the population
containing *both* answers is what makes that a control rather than a confirmation.

**This sat `open` for a day after being fixed.** The fix shipped under `fix(scripts): report which
binaries a peer actually loaded…`, a message naming no tracker entry, so nothing flipped the status
and no gate noticed — the zombie-open shape `CLAUDE.md`'s verify-open cadence exists for. It was
found by going to *implement* a fix that already existed.

Two lines in `peer-sessions.sh`, per pid:

```sh
exe=$(readlink "/proc/$pid/exe" 2>/dev/null)
[ -n "$exe" ] && [ "$(stat -c %Y "/proc/$pid")" -lt "$(stat -c %Y "$exe")" ] \
  && echo "    ^ STALE: started before its binary was built"
```

Print the binary's mtime once in the header so a reader can size the gap.

**Worth stating in the script's closing output, beside the existing authorship warning:**
a peer's numbers are evidence about the build it loaded, not about the corpus. That is the
same discipline the existing note already applies to authorship — *"do not infer authorship
from who else was present"* — extended to the other thing a peer report invites you to
infer.

**Not proposed:** killing stale servers. A live session is attached to some of these, and
this bug is about making the state legible, not about reaping it.

## The proposed fix above FAILS OPEN in exactly its target case — measured 2026-09-01

Run it against a genuinely stale process and it prints **nothing**. Verified on `pid 997544`
(started Aug 31 21:45, binary rebuilt Sep 1 00:34):

```
exe=[/home/marius/work/claude/codescout/target/release/codescout (deleted)]
(no STALE line printed)
stat: cannot statx '…/codescout (deleted)': No such file or directory
sh: [: : integer expected
```

**Mechanism.** When a process's binary has been replaced, `readlink /proc/$pid/exe` returns the
path **with a literal ` (deleted)` suffix**. `stat -c %Y "$exe"` therefore fails, the comparison
gets an empty string, `[ N -lt "" ]` errors to stderr and evaluates false, and the `&&` chain
short-circuits. The one branch that must fire is the only one that cannot. A caller redirecting
stderr — which a status script normally does — sees a clean report.

**The discriminator is already in the string the fix just read, and the fix discards it.** The
suffix *is* the answer; the timestamp comparison built on top of it is a proxy for a question
`readlink` already answered:

```sh
exe=$(readlink "/proc/$pid/exe" 2>/dev/null)
case "$exe" in
  *" (deleted)") echo "    ^ STALE: serving a binary that has since been replaced" ;;
esac
```

Shorter, correct, and it needs no binary mtime in the header — though printing one is still
useful for sizing the gap.

**Why the substitution is the interesting part, not the bug.** Start-time-versus-build-time is a
**proxy for an event the script cannot observe** — *did this process load the current bytes?* —
and it is wrong in both directions: a process started in the window between the build finishing
and the rename completing reads fresh and is not, and a rebuild producing identical bytes reads
stale and is not. `(deleted)` is not a comparison at all; it is the kernel reporting that the
inode this process holds is no longer the one at that path. That is `OB-6`'s remedy exactly —
take the signal from the **event's own side of the boundary** rather than substituting a
plausible proxy — and this bug is a worked example of it inside a *proposed fix* rather than
shipped code.

**Scope of the better instrument, stated so it is not over-trusted.** It answers *the inode this
process holds is not the one at that path*, which is the right question for a rebuild (cargo
writes and renames, so the old inode is unlinked). It will not flag a byte-identical rebuild —
harmless, since nothing is stale. It **will** flag a binary deleted for unrelated reasons, which
is also worth knowing. Linux-only via `/proc`, as the script already is.

**Live measurement, same run:** **5 of 11** `codescout start` servers were holding deleted
inodes — pids started 11:20, 11:38, 14:46, 21:45 and 22:20, all pre-dating the 00:34 rebuild;
the other 6 all started 00:34–00:35 and hold the current inode. So a third of the day's sessions
were serving replaced bytes at the moment of measurement, and every existing instrument —
`peer-sessions.sh`, `ListAgents`, the symlink at `~/.cargo/bin/codescout` (correct, and pointing
at the right path) — reported them healthy.

## Tests added

`tests/peer-sessions.sh` — 10 cases, `ce39b8f4`. Wired into a new `shell-tests` CI job in the
same commit, which is the load-bearing half: the four shell suites under `tests/` had **no
runner at all** until then. `hooks-discrimination.sh` (41 cases) and `file-provenance.sh` (58)
were cited from twelve places, every one a bug file, plan or tracker, and invoked by nothing.
A regression test with no runner would not have satisfied the archive trigger in substance.

**Cases 1 and 2 share one pid.** The same process is asserted `current`, then `REPLACED`, with
nothing changed but the file underneath it. That is what makes the pair non-vacuous by
construction — no stub satisfies both halves — and it matters here specifically because a suite
asserting only `REPLACED` passes against a function that returns `REPLACED` unconditionally.

**Mutation-verified against a real deleted binary, including the superseded fix:**

| implementation | verdict | |
|---|---|---|
| the shipped ` (deleted)` suffix check | `REPLACED` | detects |
| the fix first proposed in § *Fix* | `current` | **misses** — fail-open, reproduced |
| always `current` | `current` | **misses** |
| always `REPLACED` | `REPLACED` | caught by the intact-case assertion instead |

Two further cases: a dead pid must read `?` rather than guess in either direction, and the caller
must actually *print* the verdict — guarding the function alone would pass against a script that
computes the state and drops it, which is this corpus's `declared-not-wired` shape.

Mutations were run on temp copies, never by editing the shared checkout.

None yet — the script has no harness. If one lands, the case worth pinning is a fixture
where start time and binary mtime straddle: the shape that reads healthy today.

## References

- `R-89` — freshness breaks on build, process and distribution. This is the process axis
  made measurable on a host where the distribution axis was deliberately removed.
- `scripts/peer-sessions.sh` — the closing authorship warning this extends.
- `docs/issues/archive/2026-08-27-cross-repo-file-qualified-bucket-never-fires.md` — the
  concrete defect a stale peer would re-file.
- The measurement note in `docs/issues/archive/` on the atomic_write green-gate discharge:
  two peers' surface-size numbers were *both* correct at the instants they were taken,
  minutes apart. Same class — a stale measurement of shared state becomes confidently wrong
  by a specific amount rather than decaying into uncertainty. This bug names the instant's
  other half: which build the instant was taken on.

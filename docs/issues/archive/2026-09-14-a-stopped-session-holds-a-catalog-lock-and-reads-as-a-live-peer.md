---
kind: bug
status: mitigated
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-09-25
opened: 2026-09-14
owner: marius
related: []
severity: medium
unverified: 'TRACKED 85bb41bcd8e93d2c — the lock hold itself: a stopped holder still holds its SHARED lock (a busy_timeout stall and blocked checkpoints on this WAL catalog). Isolation is IC-17''s remedy; only the listing was fixed here.'
---

# BUG: a SIGSTOPped session holds a catalog lock indefinitely, and every instrument reports it as a live peer

## Summary

A Claude Code session on profile `~/.claude` has been in state `T` (stopped) since
2026-09-10 08:38:32. Its `codescout` MCP server is stopped with it and still holds a SQLite
SHARED lock on the production catalog. A stopped process cannot run, so it cannot release the
lock; the hold is unbounded. `scripts/peer-sessions.sh` lists it as a reachable peer with a
live socket, and nothing in the socket table, `ListAgents`, or the lock table distinguishes it
from a session that is merely busy.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `8e274b32`) — still live (E2), claim narrowed; the instance
has cleared.**
- **The instance has cleared.** § Reproduction was re-run at 07:31 +03:00. It never opens the catalog: `stat` plus
  `/proc/locks`. Pids 3031436 and 3031162 and their socket are gone, and no process on the machine is in state `T`.
  The 30 SHARED locks all belong to `Sl`/`Sl+` codescout servers, which is the baseline E1 describes.
- **E2 stands, by inspection.** `scripts/peer-sessions.sh:161` still prints no process-state column, and the script
  has had no commit since 2026-09-14.
- **Narrowed: § Root cause's "no writer can complete" does not hold for this catalog, which is WAL**
  (`PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000` at `src/librarian/catalog/mod.rs:601` and `:638`). A
  throwaway-DB probe used Python's `sqlite3`, not codescout's build, and sent SIGSTOP only to its own child. It
  measured three cases:
  - A stopped *idle* connection costs nothing.
  - A reader stopped *mid-transaction* makes the next writer wait one busy_timeout and then succeed, and blocks
    `wal_checkpoint(TRUNCATE)`, so the WAL grows.
  - Only rollback-journal (DELETE) mode reproduces "database is locked".
- **Narrowed: "stopped since Thu Sep 10 08:38:32" is the process START time.** That session's transcript has entries
  until 15:03:59 +03:00 the same day, so the stop instant was never measured.

## Symptom (Effect)

Two `cargo test --workspace` runs on this checkout stalled on 2026-09-13; one was killed to
release the other. **Whether that stall was caused by this lock is NOT established** — see
§ Root cause. What is measured here is the lock hold and the reporting gap, not the stall.

Filed now because the observation was made on 2026-09-13 and never captured; it survived only
as an `## Environment` line in two unrelated bug files and as background in
`bug-fix-session-log:F-140`. That is capture-on-notice debt, and the re-measurement below is
what it cost.

## Reproduction

    DB=~/.local/share/librarian/catalog.db
    INO=$(stat -c %i "$DB")
    grep ":$INO " /proc/locks | awk '{print $5}' \
      | xargs -I{} ps -o pid=,stat=,etime=,comm= -p {}

Any holder in state `T` is a stopped session. Resolve its owner without asking it:

    tr '\0' '\n' < /proc/<pid>/environ | grep -E '^(CLAUDE_CONFIG_DIR|CLAUDE_CODE_SESSION_ID)='

## Environment

`experiments`, 2026-09-14T10:18+03:00. 6 sessions with cwd = this checkout, 20 live overall
(socket enumeration, same instant). Catalog 418054144 bytes, inode 1023924.

## Root cause

**Unknown for the stall. Measured for the lock.**

Measured 2026-09-14T10:18: 24 POSIX ADVISORY READ locks on inode 1023924, all over byte range
`1073741826-1073742335` — SQLite's SHARED range (`SHARED_FIRST = 0x40000000 + 2`,
`SHARED_SIZE = 510`). No WRITE lock. Every holder is a `codescout` MCP server; **none is a
`cargo` or test process.**

23 holders are `Sl`/`Sl+`. One is not:

    3031436 Tl  4-01:39:43  codescout    /home/marius/.cargo/bin/codescout start --debug
    3031162 Tl              claude       /home/marius/.local/bin/claude   (its parent)

Both the MCP server and the `claude` process that owns it are stopped, since
`Thu Sep 10 08:38:32 2026`. Session `0e6223f0-d37e-48ee-9b46-4ca973fe8c8c`, profile
`/home/marius/.claude`, resolved from `/proc/3031436/environ` — a channel that needs no
cooperation, which matters because this session cannot give any.

*Inferred from the SQLite locking protocol, not measured:* a writer takes RESERVED, then
PENDING, then EXCLUSIVE over the whole SHARED range, and that last step conflicts with every
SHARED holder. So while this process stays stopped, no writer can complete. **The inference is
not the observation** — no blocked writer was observed today, and none was attributed to this
holder on 2026-09-13 either.

## Evidence

### E1 — yesterday's lock reading was the BASELINE, not a wedge signature

On 2026-09-13, "22 POSIX ADVISORY READ locks, no WRITE lock" was read as evidence that the
catalog was the thing two `cargo test` runs were wedged on. Today, on a checkout that is not
wedged and whose gate had just run green, the same query returns **24 of the same shape**.

The reading was consistent with a wedge and equally consistent with a healthy tree, so it was
never evidence about which one held. This is `bug-fix-session-log:F-140`'s shape again, one day
later and from the same session: a measurement whose value is identical in both worlds, taken
as though it discriminated. The tell is available in advance and costs one sentence — *name the
proposition, then ask what a healthy tree returns.*

### E2 — every instrument reports the stopped session as askable

`scripts/peer-sessions.sh` lists pid 3031162, and `/run/user/1000/cc-socks/3031162.sock` exists
(`srw-------`, dated Sep 10 08:38). The table's own columns are `LISTED` (visible to
`ListAgents`) and a REPLACED-binary note; neither is about liveness. So the documented remedy
for a shared-resource conflict — enumerate the peers and ask the holder — returns a party that
will never answer, and reports it in the same shape as one that will.

## Hypotheses tried

1. **Hypothesis:** the lock table shows the wedge.
   **Test:** re-ran the `/proc/locks` query on a healthy tree, 2026-09-14.
   **Verdict:** rejected — see E1. 24 locks of the identical shape, tree fine.
2. **Hypothesis:** `cargo test` holds the production catalog directly.
   **Test:** resolved every one of the 24 holders with `ps -o comm=`.
   **Verdict:** rejected for today's snapshot — all 24 are `codescout` servers, no cargo
   process among them. Says nothing about what the test binaries do while running; not measured.

## Fix

**E2 fixed 2026-09-25: the first candidate, the liveness column.** `scripts/peer-sessions.sh` now has a `STATE`
column (`ok` / `STOPPED` / `ZOMBIE` / `?`) read by a new `proc_state`. It also marks the session's codescout
server `STOPPED` in the BINARIES column, because the server is the one that holds catalog locks. When any row
cannot answer, the report prints a summary naming the next action and a party who can take it: resuming or ending
a stopped session is its operator's call, not a peer's. The state is parsed after the LAST `)` of
`/proc/<pid>/stat`, because a comm may contain spaces and `)`.

**The lock hold itself is NOT addressed, and this file is `mitigated`, not `fixed`, for that reason.** A stopped
holder still holds its SHARED lock. On this WAL catalog that costs a busy_timeout stall and blocked checkpoints
rather than a wedge (see the 2026-09-25 re-verification above). The class-level remedy is isolation, which `IC-17`
names and which this change does not attempt: IC-17's own reading is *isolate the resource, do not improve the
listing*, and this change improves the listing. What it buys is that the listing no longer tells a reader that a
party who cannot answer is askable.

Fix SHA: `4025b965` (experiments)
Patch-id: `3926b8ec3dc20b2119bb385f28ecac6fa5ad75ff`

## Tests added

`tests/peer-sessions.sh` § *one process, stopped and resumed*: ONE pid read `ok`, then `STOPPED` after SIGSTOP,
then `ok` after SIGCONT, and `?` once dead. The fixture binary is named `a) T b`, so a first-`)` parse reads `T`
from a sleeping process. There are also three wiring checks. Suite at 18/0. Mutations via `scripts/mutation-probe.sh`,
each killed:

- a first-`)` parse: 2 assertions;
- `T` read as `ok`: the STOPPED assertion;
- the server's state unwired: its wiring check.

The wiring checks are text greps, the suite's existing ceiling. The table reads the real
`/run/user/<uid>/cc-socks`, and a test cannot inject a session there.

## Workarounds

Before either test lane, `pgrep -a -x cargo` (**not** `-f 'cargo test'`, which matches the
asking shell and so can never return zero). If a lane stalls, confirm on the blocked process's
STATE — `ps -o stat=`, `/proc/locks` — never on whether its log is moving: a 69-second silence
is what *blocked* and *unblocked-and-busy* both look like (`F-140`).

## Resume

The instance has cleared (pids 3031436 and 3031162 are gone), so the operator question this section used to lead
with is moot. Nothing further is owed here. The lock-hold class is `IC-17`'s.

## References

- `docs/trackers/bug-fix-session-log.md` § `F-140` — the 69-second still-frame, whose correction
  is what made E1 findable.
- `docs/issues/archive/2026-09-14-the-compile-advisory-reports-a-cached-failure-as-current-state.md`
  § Environment — records the same contention from a peer's side, as environment rather than as
  a defect.
- `docs/trackers/issue-clusters/IC-17-shared-resource-carries-no-owner.md` — the class, and the
  source of the "isolate the resource, do not improve the listing" reading in § Fix.

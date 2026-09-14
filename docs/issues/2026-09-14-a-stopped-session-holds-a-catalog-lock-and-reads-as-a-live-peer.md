---
kind: bug
status: open
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: a SIGSTOPped session holds a catalog lock indefinitely, and every instrument reports it as a live peer

## Summary

A Claude Code session on profile `~/.claude` has been in state `T` (stopped) since
2026-09-10 08:38:32. Its `codescout` MCP server is stopped with it and still holds a SQLite
SHARED lock on the production catalog. A stopped process cannot run, so it cannot release the
lock; the hold is unbounded. `scripts/peer-sessions.sh` lists it as a reachable peer with a
live socket, and nothing in the socket table, `ListAgents`, or the lock table distinguishes it
from a session that is merely busy.

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

Not implemented, and the first step is not a code change.

**The stopped session is the operator's call and nobody else's.** Resuming it (`SIGCONT`) or
ending it are both fine; signalling another session's processes is not something a peer session
may decide, and this corpus already paid for recommending exactly that
(`bug-fix-session-log:F-140`). The question is answerable in one word, which is why it goes to
a person: *resume it, or kill it?*

Two candidate durable changes, neither argued enough to build:

- **A liveness column in `scripts/peer-sessions.sh`.** `ps -o stat=` on the pid it already has
  distinguishes `T` from `S` for free. Cheap, and it closes E2 rather than the lock hold.
- **Isolation.** A per-session or per-test catalog would make one stopped session unable to
  constrain anyone. This is the remedy `IC-17` names, and the larger of the two.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None. Nothing here is fixed yet, and both candidate fixes above are unargued — a test now would
pin a shape nobody has chosen.

## Workarounds

Before either test lane, `pgrep -a -x cargo` (**not** `-f 'cargo test'`, which matches the
asking shell and so can never return zero). If a lane stalls, confirm on the blocked process's
STATE — `ps -o stat=`, `/proc/locks` — never on whether its log is moving: a 69-second silence
is what *blocked* and *unblocked-and-busy* both look like (`F-140`).

## Resume

**Ask the operator about session `0e6223f0-d37e-48ee-9b46-4ca973fe8c8c` (pid 3031162, profile
`~/.claude`, stopped since 2026-09-10 08:38:32) before anything else.** Everything below is
downstream of that answer, and the lock cannot be cleared without it.

Then, to establish the mechanism the § Root cause leaves open — the missing observation is a
**blocked writer**, not more lock counting:

1. Reproduce a stall (two `cargo test --workspace` runs), and while it is stalled read
   `ps -o stat=,wchan= ` on the stalled process. `D` or a futex wchan is the discriminator; a
   lock count is not, per E1.
2. Check whether the test binaries open the production catalog at all —
   `ls -l /proc/<test-pid>/fd | grep catalog`. Today's snapshot says the 24 holders are all MCP
   servers, so this is genuinely open.

Do not re-derive the lock table expecting it to answer the question. It has now been read twice
and discriminated nothing.

## References

- `docs/trackers/bug-fix-session-log.md` § `F-140` — the 69-second still-frame, whose correction
  is what made E1 findable.
- `docs/issues/2026-09-14-the-compile-advisory-reports-a-cached-failure-as-current-state.md`
  § Environment — records the same contention from a peer's side, as environment rather than as
  a defect.
- `docs/trackers/issue-clusters/IC-17-shared-resource-carries-no-owner.md` — the class, and the
  source of the "isolate the resource, do not improve the listing" reading in § Fix.

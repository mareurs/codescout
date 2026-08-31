---
kind: bug
status: open
title: peer-sessions.sh collects process start time but never compares it to the binary's build time, so a peer serving stale bytes reads as healthy
tags:
- cluster/blast-radius-exceeds-visibility
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

## Tests added

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

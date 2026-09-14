---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: null
fix_patch_id: 6a84e4a2b7eaca3c25e193f20a852d588381c9c2
fix_sha: 122ea357
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: `stale-servers.sh` counts LSP muxes under a name that excludes them, and sends the reader to reconnect a process that has no session

## Summary

`scripts/stale-servers.sh` selects its population with `pgrep -x codescout`
(`scripts/stale-servers.sh:39`), which matches **LSP mux** processes as well as MCP
**servers** — a mux runs the same binary, so it carries the same process name. Every
surface that publishes the result names only servers, and the two populations have
**opposite** healing behaviour: a server never recycles, a mux self-heals at its
`--idle-timeout`. So the printed `total=` is a mixed unit, and the closing remedy —
*"Reconnect those sessions (/mcp)"* — names an action no reader can perform on a mux row.

## Symptom (Effect)

`./scripts/stale-servers.sh`, run 2026-09-14 05:45:56 +03:00 on `experiments` @ `979fe285`
(worktree clean of `src/`), printed 22 rows including this one:

```
AGE_SEC    PID       PPID      STATUS   STARTED
14         2895468   2895390   current  Mon Sep 14 05:45:48 2026
...
total=22  stale-exe=18  current=4
```

PID `2895468` is not a server. Its `/proc/2895468/cmdline`, read at the same instant:

```
/home/marius/work/claude/codescout/target/release/codescout mux
  --socket /run/user/1000/codescout-kotlin-mux-7e868829c00fa9b2.sock
  --lock   /run/user/1000/codescout-kotlin-mux-7e868829c00fa9b2.lock
  --cwd /home/marius/work/claude/codescout --idle-timeout 300
  -- kotlin-lsp --stdio --system-path=/tmp/codescout-mux-kotlin-lsp-7e868829c00fa9b2
```

Its `PPID` `2895390` is itself a `codescout start --debug` process — the mux is a **child of
another row in the same table**, counted a second time as a peer of its own parent.

**1 of 22 rows (4.5%) on this run, and 1 of the 4 `current` rows (25%).** The share is not
reliably small: memory `gotchas` records a verified partition of **9 servers + 3 muxes**
(2026-09-01) over the same predicate — 25% of the population.

## Reproduction

```
git rev-parse HEAD                 # 979fe285 at time of filing
./scripts/stale-servers.sh
for p in $(pgrep -x codescout); do tr '\0' ' ' < /proc/$p/cmdline; echo; done
```

Any row whose cmdline contains `mux --socket` is a mux the table presents as a server.
**Requires a live mux**, which exists only while a language server is warm — an LSP call in
any session in this checkout within the last `--idle-timeout` (300 s Kotlin, 180 s Rust).
With no warm LSP the script's output is accidentally correct, which is why the defect is
absent from most runs.

## Environment

Linux 7.2.3-zen1-3-zen, `experiments` @ `979fe285`, 22 `codescout` processes across
3 config profiles, 17 live sessions by socket enumeration at 2026-09-14 05:46:08 +03:00.

## Root cause

The predicate is the process **name**, and the field that distinguishes the two populations
is the **cmdline**, which the script never reads.

- `scripts/stale-servers.sh:39` — `for p in $(pgrep -x codescout 2>/dev/null); do`.
  `pgrep -x` matches the process name exactly; a server (`codescout start`) and a mux
  (`codescout mux --socket …`) both present as `codescout`.
- `scripts/stale-servers.sh:41,63` — every match increments `total`, printed as
  `total=${total}`.
- `scripts/stale-servers.sh:3` — the header asks *"Which running codescout **servers** …"*.
- `scripts/stale-servers.sh:67` — the remedy prints *"Reconnect those sessions (/mcp) to
  pick up the current binary."*

**The blind-spot list is where this should have been caught, and it runs the wrong way.**
`scripts/stale-servers.sh:22-29` documents four blind spots — unreadable `/proc`, wrapper
invisibility, deleted-inode-carries-no-version, and *"The count is a floor if a server is
mid-start"* (`:29`). All four bound the count from **below**. The mux inclusion bounds it
from **above**, and it is the one not listed — so a reader who reads the caveats carefully
concludes the number is a floor, when it is neither.

What the fix needs is already written down **next to** the instrument and was never applied
to it. Memory `gotchas` § *MCP Binary Symlink* states the partition explicitly (*"a mux is a
descendant of some server … the mux command line also carries `mux --socket …`"*, and
*"Prefer that last one — the child's OWN cmdline"*), and
`docs/trackers/bug-fix-session-log.md` § `F-108` (2026-09-03) records a session hitting the
same conflation by hand: *"The `mux --socket` processes are LSP multiplexers and are a
different population; a first pass counted them together and got 22, which is why the unit
is stated here."*

Measured 2026-09-14 05:45:56: `./scripts/stale-servers.sh` → `total=22`; the same population
filtered on `mux --socket` → **1**.

## Evidence

### The remedy is unperformable on a mux row, in both directions

A mux has no Claude Code session and no `/mcp`: it is spawned by a server, keyed by project
hash, and **shared across sessions**. There is nothing to reconnect. The action is also
unnecessary — the mux's own cmdline carries `--idle-timeout 300`, and memory `gotchas`
records the consequence: *"A stale mux self-heals once idle past its `--idle-timeout` (180s
Rust, 300s Kotlin), so the exposure is use-coupled."* A stale server never recycles, because
`codescout start` takes no idle-timeout option.

So: one table, one remedy sentence, two populations whose correct next actions are
*"reconnect that session"* and *"do nothing"*. Nothing in the output tells them apart.

This is the failure CLAUDE.md § *Testing Discipline* names — **name the next action the
message produces and ask whether that party can perform it.** Here the party does not exist.

### The published baseline disagrees with itself by one, across three surfaces

All three cite the same date and the same *"oldest 17 days"*:

| surface | figure |
|---|---|
| `scripts/stale-servers.sh:9` | *"Measured 2026-08-21 on this machine: **22 of 26** servers stale"* |
| `docs/PROBES.md:180` | *"Baseline 2026-08-21: **21 of 26** stale, oldest 17 days"* |
| `docs/RELEASE.md:171` | *"Measured 2026-08-21: **21 of 26 servers** … the oldest 17 days old"* |

A reader cannot tell which is authoritative, and the measurement is irreproducible. Whether
the off-by-one *is* a mux counted in one reading and not the other is a plausible
fingerprint of this bug and is **not** established — see Hypotheses tried #2.

## Hypotheses tried

1. **Hypothesis:** the script already filters muxes and PID 2895468 was misread.
   **Test:** read `scripts/stale-servers.sh` in full; grep it for `mux`.
   **Verdict:** rejected — the file contains no occurrence of `mux`, and `:39` is an
   unfiltered `pgrep -x codescout`.
   **Evidence:** § Root cause.

2. **Hypothesis:** the 22-vs-21 baseline split is this defect's fingerprint — one surface
   transcribed a total that included a mux.
   **Test:** none available; the 2026-08-21 process table is gone.
   **Verdict:** deferred — irreproducible by construction. Recorded so a later reader does
   not mistake the coincidence for a confirmation.

3. **Hypothesis:** muxes are rare enough that the conflation is immaterial.
   **Test:** compare this run against the recorded partition in memory `gotchas`.
   **Verdict:** rejected — 1 of 22 today, but 3 of 12 (25%) on 2026-09-01. A mux exists
   exactly while an LSP is warm, i.e. during active source work, which is when someone is
   most likely to run this.

## Fix

Shipped. The sketch below is kept as written, because the order it proposed is the order it was
done in and a reader comparing plan to outcome should see both:

1. `scripts/stale-servers.sh:39-41` — read `/proc/$p/cmdline` once per pid and classify on
   `mux --socket`. Memory `gotchas` verified that partition against every live mux and
   prefers it to the parent-lookup test, which carries a latent false positive on this
   repo's own path. Print the kind as a column; report `servers=N muxes=M` rather than one
   `total=`.
2. `scripts/stale-servers.sh:65-68` — split the remedy by kind. Servers → *"reconnect that
   session (/mcp)"*. Muxes → *"no action; it self-heals after `--idle-timeout`"*, which is
   readable off the cmdline the classifier already parsed.
3. `scripts/stale-servers.sh:22-29` — add the blind spot, and say that the existing four
   bound the count from below while this one bounds it from above.
4. `docs/PROBES.md:180` and `docs/RELEASE.md:171-174` — state the unit on the read surface.
   Per CLAUDE.md § *Observer Blindness* position 3, the scope belongs where the number is
   read, not only in the script that computes it.

Do **not** re-derive the 2026-08-21 baseline to settle the 22/21 split — that process table
is gone. Make the surfaces agree on one figure, labelled as computed under the old
conflated unit, or drop it.

**Fixed on `experiments` by `122ea357`** — SHA is positional and dies on rebase; the durable
identifier is the patch-id below.

**SHA:** `122ea357` (on `experiments`). **patch-id:** `6a84e4a2b7eaca3c25e193f20a852d588381c9c2`.

**NOT ARCHIVED, and the reason is a gap rather than a formality.** CLAUDE.md's archive trigger
is *gate green plus a regression test*. The regression test exists; the gate is not green. The
fourth command (`cargo test --workspace`) never completed: two attempts wedged at the identical
point (5529 lines, no `test result` block), main thread futex-parked, killed after ~12.5 minutes
of zero output. `cargo test --workspace` opens the real
`/home/marius/.local/share/librarian/catalog.db` with 14-17 fds — not an isolated fixture — and
22 processes hold POSIX advisory READ locks on it with no WRITE lock anywhere, one of them a
codescout server SIGSTOPped since 2026-09-10 whose parent `claude` is also stopped.

**CORRECTED 2026-09-14 07:20, and the correction is the useful part.** The two sentences that
stood here — *"a peer's gate wedged identically and did not resume when this session's run was
killed, so the two were not deadlocking each other"* — are **false**, and they also went into the
commit messages of `122ea357` and `6b2a6b0d`, which are not amendable on a shared tree. What
actually happened: this session's run was killed at 06:46:40; the peer's gate log was checked at
**06:47:49, sixty-nine seconds later**, showed no movement, and that still-frame was written down
as a negative result. The peer's gate then resumed and finished `DEFAULT_EXIT=0` at 06:51:00.

So the two runs **were** deadlocking each other, in all likelihood, and killing one freed the
other — stated at its real strength, the timing is consistent rather than proven. What **is**
proven is the falsification of the hypothesis this file pointed at: the SIGSTOPped process held
its shared lock throughout and the peer's run completed anyway, so it was never the cause.
(`bug-fix-session-log:F-140`.)

**Also checked, and NOT a defect:** `fmt-mine.sh` refused (exit 1) during this gate run, naming
two peer-owned files. Both are now `rustfmt --check`-clean and match HEAD — peer commit
`d311762a` landed them — so that refusal was correct when it fired and is merely stale. Run fresh
it refuses on a *different* peer's file, because the refusal set rotates with whoever is holding
uncommitted Rust. This change contains no Rust. clippy 0, lean 0.

## Tests added

[`tests/stale-servers.sh`](../../tests/stale-servers.sh) — **18 cases**, wired as its own CI job
(`stale-servers-tests` in `.github/workflows/ci.yml`; its own job because `shell-tests` is a
known-flaky lane and this suite's assertions are verdicts about a count's UNIT).

Driven through the two seams the fix added rather than through live processes, and that is the
design: a mux exists only while a language server is warm, so a suite that waited for one would
inherit the exact intermittency that hid this defect and would pass silently in CI, where no mux
ever exists. `--classify` calls `kind_of_cmdline` and `--remedy` calls `remedy` — the same two
functions the live loop calls, so the suite drives production and not a re-implementation of it.

**Mutation run against the production path at fix time — 4 of 4 killed**, and the per-mutation
kill counts are the informative part, because two of them died to exactly one case each, which is
what says those cases are not redundant with the rest:

| mutation of `scripts/stale-servers.sh` | cases killed |
|---|---|
| classifier narrowed to `*mux*` (drop the ` --socket`) | 1 — the `/opt/tmux-tools/` control, alone |
| classifier stubbed to answer `server` unconditionally | 4 |
| delete the `remedy`'s mux branch | 3 |
| reinstate a combined `total=` line | 1 — the absence assertion, alone |

The absence assertion (*no combined total is printed*) is monotone under removal — deleting the
whole summary satisfies it — so it is paired on the line with the two presence assertions for
`servers=` and `muxes=`. Neither half is worth anything alone, and the suite says so at the site.

What these do **not** cover: the `/proc` read itself, and the live loop's counters. Those are
exercised only by the one live invocation, whose value is population-independent (on a runner
with no codescout process the counters print zero and every assertion still holds) — which is
what makes it CI-safe and also what makes it thin. A regression that broke only the `/proc`
reading would show up as an empty table, not as a failure.
## Workarounds

Filter by hand before trusting the count:

```
for p in $(pgrep -x codescout); do
  cmd=$(tr '\0' ' ' < /proc/$p/cmdline)
  case "$cmd" in *"mux --socket"*) echo "MUX    $p" ;; *) echo "SERVER $p" ;; esac
done
```

Rows the script marks STALE that appear as `MUX` need no action.

## Resume

**One step owed, and it is the archive blocker:** run `cargo test --workspace` to completion and
confirm it is green, then archive via `doc(action="move", …)` — never a bare `git mv`.

At 07:17 that lane failed in 37 lines for a reason unrelated to both this fix and the earlier
wedge: a peer's uncommitted `src/util/path_security.rs` references `extract_grep_pattern`, which
does not exist yet (5 × `E0425`). `run_command`'s `wip_authors` hook named the holder
automatically. That is ordinary shared-checkout churn — ask, never fix — and it clears when they
land. Retry then.

**Do not chase the SIGSTOPped process this section used to name as the lever.** That hypothesis is
falsified; see § Fix. The earlier wedge is best explained by two concurrent `cargo test
--workspace` runs on the shared production catalog, and the cheap avoidance is not to start one
while another session's gate is running — `pgrep -a -f 'cargo test'` answers that in one call.

Nothing about the fix itself is outstanding. If a second instance of the defect CLASS appears,
the roster's `cluster/unclassified` `**Members:**` field is where the adjudication is parked —
see § Defect class.
## References

- `scripts/stale-servers.sh` — the instrument
- `docs/PROBES.md:180` — index row; its blind-spot column omits muxes
- `docs/RELEASE.md:171-174` — cites the baseline in the ship sequence
- memory `gotchas` § *MCP Binary Symlink* — the verified server/mux partition, and the
  reason to prefer the child's own cmdline over a parent lookup
- `docs/trackers/bug-fix-session-log.md` § `F-108` — a session hitting the same conflation
  by hand on 2026-09-03 and stating the unit in prose
- `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`
  — the impact half: why a stale process matters at all

## Defect class

Filed `cluster/unclassified`, deliberately. Every class in
[`docs/trackers/issue-clusters.md`](../trackers/issue-clusters.md) addressing a
name-vs-population mismatch is oriented the **narrow** way — `IC-14` *"a guard's coverage is
narrower than its name"*, `IC-18` *"a selector is narrower than the population it names"*,
`IC-20` *"a floor is published under the name of a total"*. This is the mirror: the selector
is **wider** than the population its name denotes, so the number is a ceiling rather than a
floor, and the remedy is to narrow or split rather than to widen.

`IC-20` is the near miss, and the misfit is specific: its claim turns on the walk having
**stopped**, which makes the true value *"not merely unreported but unknowable"*. Here the
walk over-collected and the true value is one `cmdline` read away. Tagging it there would
dilute exactly the clause that keeps it apart from `IC-19`.

Whether that mirror is its own class or a widening of `IC-18` is a promotion judgement, and
one instance does not reach the bar (≥3 instances spanning ≥2 subsystems). Recorded here so
the second instance has something to find.

**Prior art, found by the check this session shipped a lesson about.** The roster's
`cluster/unclassified` `**Members:**` field already carries one instance of this exact
direction — `worktree-guard-word-boundary-blocks-read-only-git-plumbing` (2026-09-03), where
`git-worktree-guard.mjs:65` terminates each destructive verb with `\b`, a hyphen is a word
boundary, and so `merge\b` matches `git merge-base`. That filer reasoned to the same place —
*"`IC-14` is a guard whose coverage is NARROWER than its name — this one's is WIDER"* — and
left an explicit marker: *"Worth recording for whoever meets the second instance: … The
candidate class, once a second instance exists, is 'a guard's trigger matches a superset of
what its name claims'."*

This is that second instance, and it is **not obviously the same consequence**. Theirs
refuses work that should pass; this one returns a number under the wrong unit and routes the
reader to a party that does not exist. Superset-of-trigger is common to both;
misrouted-remedy is only here. Whether the class is the trigger shape or the trigger-plus-
consequence shape is a promotion judgement a third instance should settle, and this file
deliberately does not settle it — `IC-11` was promoted early *"forced by a taggable instance
arriving against a gate with no `cluster/unclassified` escape hatch, rather than by its
count"*, and that cost is recorded in the roster.

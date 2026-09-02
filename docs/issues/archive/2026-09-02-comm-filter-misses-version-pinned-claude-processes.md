---
id: d2ad0e747aaed72d
kind: bug
status: fixed
title: the self-identification walk filters on comm, so a version-pinned session cannot find itself
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-03
unverified: NO REGRESSION TEST, and this is the half that has none. The walk reads `/proc` and `/run/user/<uid>/cc-socks` by hardcoded absolute path, so it cannot be pointed at a fixture without editing the skill, and a source-text assertion ("the block contains no comm test") is a proxy for the behaviour rather than the behaviour. Verified by hand against one live version-pinned process; that verification is not repeatable and the population is transient — 3 such sessions existed on 2026-09-02, 1 on 2026-09-03.
---

## Summary

`/codescout-companion:reaching-peer-sessions` Step 1 identifies the caller's own session by
walking up the process tree until it finds a process named `claude`:

```
me=$$
while [ "$me" -gt 1 ] && [ "$(cat /proc/$me/comm 2>/dev/null)" != claude ]; do
  me=$(awk '/^PPid:/{print $2}' /proc/$me/status 2>/dev/null) || break
done
```

`comm` is the executable's basename. The standard versioned install puts the binary at
`~/.local/share/claude/versions/<version>`, so for those sessions **`comm` is the version
string**, not `claude`. The walk passes its own server without recognising it and runs to
PID 1, and the `<-- you` marker — computed as `[ "$p" = "$me" ]` — never matches any row.

The table is otherwise correct. Every peer enumerates, every column is right; the only thing
missing is the row that says which one is the reader. Nothing marks the omission, because the
loop terminated normally.

## Symptom (Effect)

**A session that cannot find itself in the table cannot subtract itself from the count.** That
is precisely the off-by-one `CLAUDE.md` § *Reaching a Peer Session* records as having shipped
three times in one evening, twice inside a correction of itself, and which the skill's own text
exists to prevent:

> Sessions and peers differ by one — yours.

The skill deliberately rejects the cheaper `pgrep … | head -1` for self-identification, on the
correct ground that it "samples arbitrarily among several running servers". The reasoning is
sound; the replacement's filter is what is narrow.

## Reproduction

Measured 2026-09-02 on this machine.

| population | count |
|---|---|
| socket-bound live PIDs | 21 |
| …with `comm` = `claude` | 18 |
| …with `comm` = `2.1.258` | **3** |

All three are in this checkout, all on the `.claude` profile, and `readlink /proc/<pid>/exe`
gives `/home/marius/.local/share/claude/versions/2.1.258` for each. For those three the walk
terminates at PID 1 and prints no `<-- you` row.

This session's own walk *succeeds* — terminating at PID 3411389, `comm=claude` — which is why
the defect is invisible from here. Whether a reader is affected depends on how their binary was
installed, and nothing in the output distinguishes "no `<-- you` row because I am not in this
list" from "no `<-- you` row because my process is named after its version".

## Root cause

A selector keyed on `comm`, a value nothing promises will be `claude`. `comm` is derived from
the executable name and is additionally truncated to 15 bytes by the kernel, so a
version-pinned install, a wrapper script, a symlink under another name, or a sufficiently long
path component all defeat it. The structural read is `readlink /proc/<pid>/exe`, which names
the binary regardless of what the process is called.

## Evidence

### The second-order effect is the reason this is an entry and not a footnote

The same `comm` selector is what an author reaches for when *measuring* this population, and it
silently narrows those measurements too. Session `f13f8169-93a1-4392-95d1-8774d296e0c0`
derived the live-but-socketless residual for
`docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md`
as **2**, via `pgrep -x claude`. Re-derived structurally by exe path:

| instrument | live claude processes | socketless |
|---|---|---|
| `comm` filter (`pgrep -x claude`) | 20 | 2 |
| socket enumeration | 21 | n/a |
| **structural, by `readlink /proc/<pid>/exe`** | **28** | **7** |

So the residual is **7, not 2** — and the figure that understated it was produced by the very
selector this file is about, while measuring a different bug in the same script. A narrowed
selector does not only narrow the feature it guards; it narrows every measurement anyone takes
with the same idiom, including the measurement of its own blast radius.

### Why this is filed apart from the greedy-regex bug

Both defects live in Step 1 of one script, and the peer who found this one proposed folding it
into the existing file to avoid fragmenting the class. The ledger's own ruling goes the other
way — *if a finding satisfies a second class's claim, it is a second bug file* — because one
file carries exactly one `cluster/` tag and the buried half is systematically the one nearest a
threshold.

The two are different classes. The greedy regex is `IC-6`: a scheme with no way to disambiguate
two tokens that collide, silently binding to the wrong one. This one matches too **little** —
`IC-18`'s claim verbatim. `IC-18`'s own Members list already contains this exact adjudication
(`declared-patch-ids-per-line-scan-misses-a-wrapped-value`, *"Filed here and not `IC-6`, which
a peer proposed"*), which is the second time a peer has proposed `IC-6` for a too-narrow
selector.

## A second site, which this file did not name

This file located the `comm` filter at `reaching-peer-sessions/SKILL.md` Step 1 and stopped there.
The identical test — `[ "$(tr -d '\0' </proc/$p/comm)" = claude ]` — also sat in a
`reconnaissance-patterns.md` R-N entry, where it served as a **liveness gate** rather than a
self-identification walk. Same selector, worse failure: a version-pinned session is skipped, the
loop prints nothing, and a valid sessionId resolves to *"no such session"* rather than to an
error. The narrowing produced a confident negative.

Found by grepping the tree while fixing the first site, not by reading this file, which named one
and read as complete — `bug-fix-session-log:W-102` holding about the bug file that helped produce
it. Corrected in place, with the broken original preserved and marked *do not copy*.
## Proposed fix

Identify the server by **exe**, not by name:

```
while [ "$me" -gt 1 ]; do
  case "$(readlink /proc/$me/exe 2>/dev/null)" in
    */claude|*/.local/share/claude/versions/*) break;;
  esac
  me=$(awk '/^PPid:/{print $2}' /proc/$me/status 2>/dev/null) || break
done
```

A cheaper and more robust alternative exists and should be preferred if it holds: the walk's
purpose is only to find which socket-bound PID is an ancestor of this shell, so it can
intersect the ancestor chain with the socket list directly and never inspect a name at all.
That has no selector to get wrong.

**Whatever the fix, make a failed self-identification loud.** The defect's whole cost is that
`me=1` is indistinguishable from a correct run in the output. If no row matches, the table
should say so rather than printing silently — a count derived from a table with no `<-- you`
row is off by one and the reader has no way to know.

## Status

**Fixed** at `claude-plugins:bb14719`, patch-id `87019883ed5b6a85ae30999f6cc3381522fc73dc`
— the same commit as the sibling greedy-regex bug, cross-repo so the SHA is prefixed.

The walk no longer asks what a process is *called*. It climbs to the first ancestor **holding a
socket in `cc-socks`**, which is what "a claude server" means here and is a property the kernel
records rather than a name the binary happens to carry — the general form recorded in
`IC-18`'s `**Members:**`.

**Verified 2026-09-03 on the discriminating case**, which is the only one that proves anything:
this session's own `comm` is `claude`, so its `<-- you` row worked *before* the fix and is
monotone under the defect. Against live pid `985365` (`comm=2.1.258`, exe
`~/.local/share/claude/versions/2.1.258`) the two walks diverge as predicted — the old one passes
over it and runs to PID 1, printing no `<-- you`; the new one terminates there.

**The loud-failure half shipped too**, which was the recommendation this file made independently
of any particular fix: when the walk finds no socket-bearing ancestor the summary now drops the
peer figure entirely and says why, rather than printing a session count a reader silently uses as
a peer count.

See `unverified:` — this half has no regression test and the reason is structural, not neglect.

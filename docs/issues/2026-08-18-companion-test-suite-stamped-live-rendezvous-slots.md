---
status: fixed
opened: 2026-08-18
closed: 2026-08-18
severity: high
owner: marius
related: []
tags: [companion-plugin, test-isolation, rendezvous, xdg]
kind: bug
---

# BUG: the companion hook's test suite stamped LIVE rendezvous slots, because `${VAR-}` is not `${VAR:-}`

## Summary

`codescout-companion/hooks/session-start.test.sh` ran the SessionStart hook against the
developer's real `$HOME/.local/state/codescout/servers` instead of a temp directory, because
its `ctx()` helper used `${XDG_STATE_HOME-}` (dash) rather than `${XDG_STATE_HOME:-…}`
(colon-dash). With the variable unset, `-` yields the **empty string**, so `XDG_STATE_HOME=`
was exported and the hook fell back to `~/.local/state`. One test run overwrote the
rendezvous slots of three codescout MCP servers — one of them live, under a live `claude`
session — with the fake session id `sst-startup`. The suite performed exactly the
cross-window cross-stamp the rendezvous feature exists to prevent.

## Symptom (Effect)

After running `bash hooks/session-start.test.sh`, the real per-user rendezvous directory
contained three stamped slots with an identical `hook_at` to the nanosecond — the signature of
a single hook invocation:

```
/home/marius/.local/state/codescout/servers/782781.json
/home/marius/.local/state/codescout/servers/786032.json
/home/marius/.local/state/codescout/servers/789484.json

{"pid":789484,"ppid":2647624,"started_at":"2026-08-18T14:36:08.499442589Z",
 "cwd":"/home/marius/work/claude/codescout",
 "session":"sst-startup","hook_at":"2026-08-18T15:44:58.106Z"}
```

`sst-startup` is the fixture id the test's `ctx()` generates (`sst-<source>`); no real
conversation is ever named that.

## Reproduction

Companion repo at `95e8c85`, codescout at `feb845aa` (branch `experiments`).

1. Ensure `XDG_STATE_HOME` is unset and at least one codescout MCP server is running.
2. `cd codescout-companion && bash hooks/session-start.test.sh`
3. `ls ~/.local/state/codescout/servers/` — slots now carry `"session":"sst-startup"`.

8 of the 9 `ctx` invocations reproduce it (`test.sh:89, 90, 108, 139, 149, 158, 173, 182`);
only the one at `:39` set `XDG_STATE_HOME` explicitly and was therefore isolated.

## Environment

Linux, bash. Companion repo `/home/marius/work/claude/claude-plugins/codescout-companion`.
Affects any machine where codescout is running, since `tests/run-all.sh:12` globs
`codescout-companion/hooks/*.test.sh` and that gate runs before every version bump.

## Root cause

POSIX parameter expansion: `${VAR-word}` substitutes `word` only when `VAR` is **unset**,
whereas `${VAR:-word}` substitutes when `VAR` is unset **or empty**. The helper read

```bash
| XDG_STATE_HOME="${XDG_STATE_HOME-}" node "$HOOK" 2>/dev/null \
```

so with `XDG_STATE_HOME` unset the expansion produced `""` and exported an *empty* value.
The hook's own resolution (`session-start.mjs:57`, `process.env.XDG_STATE_HOME || join(home,
'.local','state')`) treats `""` as falsy and falls back to the real home — correctly, by its
own logic. Nothing was wrong with the hook; the test handed it a poisoned environment.

measured 2026-08-18: `ls -la ~/.local/state/codescout/servers/` showed the three stamped
files; `/proc/789484` present with `cmdline` = `codescout start --debug`, and its parent
`/proc/2647624` present as a `claude` process — so the corrupted slot belonged to a live
server on a live session, not to a test fixture.

## Evidence

### The live server whose slot was overwritten

```
789484 ALIVE: /home/marius/.cargo/bin/codescout start --debug
2647624 ALIVE: /home/marius/.local/share/claude/versions/2.1.233 --session-id 44c01c0f-…
```

### Why it would have misfired

`Rendezvous::poll` (`src/tools/rendezvous.rs:120-141`) short-circuits on unchanged mtime; the
stamp moved it, so the next guide-eligible call on that server would parse the slot, find
`session: "sst-startup"` differing from its own key, and `rekey` the guide ledger — discarding
that conversation's dedup state and re-injecting every guide body. That is the safe direction
under the design's governing invariant ("degrade to re-sending, never to suppressing"), so the
blast radius is wasted tokens in someone else's conversation rather than suppressed guidance.

## Hypotheses tried

1. **Hypothesis:** the hook resolves the state dir incorrectly.
   **Test:** read `session-start.mjs:57` and compare with `src/util/fs.rs:108-113`.
   **Verdict:** rejected — the hook's `||` fallback is correct for an empty string; the defect
   is upstream, in what the test exported.
2. **Hypothesis:** only the one explicitly-set invocation was isolated.
   **Test:** counted `ctx` call sites and checked which set `XDG_STATE_HOME`.
   **Verdict:** confirmed — 1 of 9 isolated, 8 leaked.

## Fix

`codescout-companion:b8ffa8b` — `hooks/session-start.test.sh:29` now uses
`XDG_STATE_HOME="${XDG_STATE_HOME:-$TMP/state}"`, so an unset *or* empty value falls back to
the per-run temp dir. The same commit aligned `session-start.mjs` with `src/util/fs.rs`'s
XDG-spec handling, so a *relative* value is now treated as unset on both sides.

**Remediation of the damage** (2026-08-18, this session): the three corrupted slots were
backed up and removed. Deletion was chosen over repair for the live one because
`poll`'s `std::fs::metadata(path)…ok()?` returns `None` on a missing file, degrading to
exactly the documented no-hook path, whereas leaving the file guaranteed one spurious
full re-injection. Verified after the fix: the directory is empty and a full suite run no
longer creates anything in it.

## Tests added

`hooks/session-start.test.sh` — the suite now asserts the real state directory is untouched
after a run. Verified by the controller independently: `~/.local/state/codescout/servers/` is
empty, mtime unchanged, after the post-fix suite run.

## Workarounds

Export `XDG_STATE_HOME=$(mktemp -d)` before running the companion hook tests on any machine
with a live codescout server.

## Resume

N/A — fixed and verified. Not archived yet: the fix lives in the **companion** repo, so it is
not on codescout's `experiments` branch, and this file's archive gate is written for
codescout-side fixes. Archive once the guide-ledger Phase B whole-branch review closes, and
record the SHA with the cross-repo `codescout-companion:` prefix rather than a bare hash.

## References

- `docs/superpowers/plans/2026-08-18-guide-ledger-phase-b-identity.md` — Task 6, where it was found
- `docs/architecture/companion-plugin.md` — hook inventory
- `src/tools/rendezvous.rs` — the slot format and `poll`'s mtime short-circuit
- `src/util/fs.rs:108-113` — the XDG-spec handling the hook now mirrors

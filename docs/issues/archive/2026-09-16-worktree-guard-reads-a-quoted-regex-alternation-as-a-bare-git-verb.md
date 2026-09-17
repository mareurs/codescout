---
id: 15c6773f542cc9f0
kind: bug
status: fixed
title: The worktree guard reads a quoted regex alternation as a bare git verb, and neither printed escape applies
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-17
---

# BUG: The worktree guard reads a quoted regex alternation as a bare git verb, and neither printed escape applies

## Summary

`git-worktree-guard.mjs` splits a command on shell separators **quote-naively**, so a
regex alternation inside a quoted argument — `grep -E "git push|peter-evans"` — is split
at the `|`. The resulting fragment ends in `git push`, which satisfies `TRIGGER`'s
end-of-segment boundary, and a read-only `grep` is refused as a destructive git mutation.

The sharper half is the refusal text: both escapes it prints are addressed to someone
running a git command. The refused party ran a `grep`. `git -C <path> <verb>` is not
expressible for a `grep`, and the `cd`-chain form is stripped before the hook sees it
(`docs/issues/2026-09-13-worktree-guards-cd-chain-remedy-is-stripped-before-the-guard-sees-it.md`).
**A reader who follows the message has no working option.**

## Symptom (Effect)

```
⛔ Worktree-ambiguous git mutation. BLOCKED.

Command : echo "=== workflows that push ==="; grep -rn -E "git push|peter-evans|create-pull-request|GITHUB_TOKEN" .github/workflows/ 2>/dev/null | grep -v "^\s*#" | head -20; ...
Offender: grep -rn -E "git push
```

`Offender:` is the whole finding — it is not a command, it is the left fragment of one.

Denial is signalled by the PreToolUse JSON payload, **not** by exit status: the hook
exits `0` on both verdicts, so exit code does not discriminate.

## Reproduction

Measured 2026-09-16 against `1.20.11`, driving the hook on its real stdin contract
(`{tool_name, tool_input.command, cwd}`) so no shell runs:

| # | command | verdict |
|---|---|---|
| 1 | `grep -rn -E "git push\|peter-evans" .github/workflows/` | **BLOCKED** |
| 2 | `grep -rn "git push" .github/workflows/` | allowed |
| 3 | `cd <repo> && grep -rn -E "git push\|peter-evans" .github/workflows/` | allowed *(in-process; see Note)* |
| 4 | `git push origin experiments` | **BLOCKED** — control, guard works |
| 5 | `ls -la docs/` | allowed — control |

**Case 2 is the load-bearing one**: identical text, alternation removed, and it passes.
That isolates the `|` as the sole cause rather than the substring `git push`. Without
the split, the character after `push` is `"`, which satisfies neither `\s` nor `$`.

Present in **every** cached version `1.20.0` → `1.20.11` (12/12 block case 1). Not a
regression; it has never worked.

**Note on case 3:** the `cd` exemption works when the hook is driven directly, which is
why the real incident still blocked — the harness lifts a leading `cd <path> &&` out of
the string before the hook runs. This bug and the `cd`-strip bug compose: the exemption
that would mask this one is removed one layer up.

## Environment

Linux; `codescout-companion` `1.20.11`, confirmed active via `installPath` in
`plugins/installed_plugins.json` for all three profiles (no cross-profile drift).
Repo `codescout`, branch `experiments`, 9 linked worktrees — the `>=2 worktrees`
carve-out does not apply.

## Root cause

`hooks/git-worktree-guard.mjs`:

- `segments()` splits on `/(\|\||&&|;|\||\n)/`. Quote-naive **by design**, with the
  stated rationale: *"a mis-split only ever produces SMALLER segments, which makes an
  exemption less likely to co-occur with a trigger — i.e. it fails toward blocking,
  never toward allowing."*
- `TRIGGER = /git\s+(commit|push|reset\s+--hard|rebase|merge|checkout\s+-b)(\s|$)/`.

The rationale is **true as written and still produces this defect**: it prices a
mis-split as safe because it cannot let a mutation through. It does not price the false
refusal of a read-only command. The mis-split does not merely shrink the segment — it
**manufactures the `$` boundary** that `TRIGGER` requires, so the split is not a neutral
narrowing, it is the thing that completes the match.

Note the guard already strips heredoc bodies (`stripHeredocs`) on the explicit ground
that *"their content is data, not syntax."* Quoted strings are the same claim and are
not covered. This is `CLAUDE.md` § *Parsers Over a Namespace* — the *no escape* half:
a git verb cannot be **mentioned** inside a quoted pattern, only **used**.

The `\b` to `(\s|$)` boundary fix from
`docs/issues/archive/2026-09-03-worktree-guard-word-boundary-blocks-read-only-git-plumbing.md`
does **not** reach this: that fix stops a hyphen ending a verb, while here the segment
boundary itself is the terminator.

*measured 2026-09-16: 6-case probe + a 12-version sweep, driving the hook on stdin.*

## What is NOT established

- **Which layer strips the leading `cd`** — inherited from the `2026-09-13` bug, still open.
- **The discriminating test in that bug remains unrun.** It requires a guarded verb with a
  `cd` to a *different* worktree; the auto-mode classifier denied it here
  (`[Irreversible Local Destruction]`) before the worktree guard was reached. A second
  gate refusing first is itself the *"a case only exercises the guard it NAMES if every
  OTHER guard admits its input"* law in § *Testing Discipline*.
- **Frequency.** Not counted. Any `grep -E` over docs or CI for git operations is a
  candidate, which is a common shape in this repo, but no census was run.

## Cost

Low per occurrence, but the recovery path is the expensive part: neither printed escape
applies, so the reader must infer that the guard misparsed a non-git command. In the
incident that produced this file the workaround found was to reword the pattern to avoid
the alternation. `grep -e pat1 -e pat2` also avoids it.

## Candidate fixes (not implemented)

**Candidate 1 shipped.** The other two were not needed: anchoring `TRIGGER` to segment start
would have been a second mechanism for the same claim, and the refusal text is left alone
because the parse it would have explained no longer happens.

1. **Strip quoted spans before segmenting**, as `stripHeredocs` already does for heredocs
   — same claim (data, not syntax), same place.
2. **Require the segment to start with the verb**: anchor `TRIGGER` so `git` must be at
   segment start (modulo env-var prefixes), since `grep -rn -E "git push` has it medially.
3. **Make the refusal name the parse**: echo the segment *and* say it was read as a
   command, so a reader whose command is not a git command can see the misread. This is
   the § *Testing Discipline* remedy-text law — the predicate is tested, the remedy text
   is not, and here the remedy text addresses the wrong party entirely.

## Prior art

Five worktree-guard bugs precede this; four archived and fixed, one open.

- `docs/issues/2026-09-13-worktree-guards-cd-chain-remedy-is-stripped-before-the-guard-sees-it.md` — **open**, composes with this one
- `docs/issues/archive/2026-09-03-worktree-guard-word-boundary-blocks-read-only-git-plumbing.md`
- `docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md`
- `docs/issues/archive/2026-09-02-a-git-verb-regex-swallows-longer-subcommands-sharing-its-prefix.md`
- `docs/issues/archive/2026-09-01-staging-op-reads-a-detached-flag-value-as-the-subcommand.md`

Closest is the archived whole-command-scan fix cited in the guard's own header: it moved
detection from whole-string to per-segment and added heredoc stripping. **This bug is the
residue of that fix** — the segmenting it introduced is the mechanism here.


## Fix provenance

Fixed in the **`claude-plugins`** repo rather than this one — the guard is a
`codescout-companion` hook — so the citation carries the cross-repo `<repo>:` prefix.

- **SHA:** `claude-plugins:579b9c1` (main) — positional; does not survive a rebase.
- **patch-id:** `e57bce0f2eb4c93efad86c3094b85b233af58850` — content hash of the diff; survives rebase and cherry-pick.

`stripQuoted()` collapses each complete `'…'` / `"…"` span to one inert token before
segmenting, mirroring the claim `stripHeredocs` already makes about heredoc bodies. A token
rather than a deletion, because `git -C "<path>" commit` and `cd "<path>"` need the path to
survive as a single `\S+` word.

**Shipped, not merely committed** — the distinction this corpus pays for repeatedly. Released
as `codescout-companion` 1.20.12 (`claude-plugins:c769ea4`), seeded into all three profile
caches with install records repointed, parity checks green. Verified **live** after
`/reload-plugins`: the exact command in § *Symptom* now runs, and both directions were
re-probed against the cached bytes the running instance actually loads — 6/6, zero
mismatches. The `cache = working tree` column was checked with a control pair rather than a
presence grep (`function stripQuoted` is 1× in each 1.20.12 cache, 0× in the 1.20.11 cache
beside it), so that green is known to be able to read otherwise.

One profile's live state is **not** established: `/reload-plugins` was run in `~/.claude-kat`
only. Sessions in `~/.claude` and `~/.claude-sdd` hold the pre-fix hook in memory until they
reload, since CC resolves hook commands at process launch.

If the SHA stops resolving, recover the commit by patch-id.

## Tests added

Five, in `claude-plugins:tests/test-git-worktree-guard.sh`, all written **before** the fix
and three observed RED (40 passed / 3 failed, each showing `Offender: grep -rn -E "git push`).

Must ALLOW — the direction the suite had no coverage for:

- a git verb inside a **double-quoted** regex alternation
- the same inside a **single-quoted** alternation
- a chained `cd` whose quoted path contains a **space**. The space is load-bearing and the
  first version of this fixture lacked it: an unspaced quoted path already satisfies `\S+`
  quotes-and-all, so that version passed *pre-fix* and was monotone under the absence of the
  thing it existed to pin. Caught only because it passed on its first run
  (`shell-gating-session-log:W-3`).

Must still DENY — without these the fix trades a false refusal for a missed mutation, which
is the direction that actually matters:

- a bare mutation following a quoted alternation
- a bare mutation following an **unterminated** quote

Suite after: **43 passed, 0 failed**. Full `claude-plugins:tests/run-all.sh` green.

Note what the pre-existing suite did and did not cover: three tests asserted *a quoted
mention must not DISARM the guard*, none asserted *must not TRIGGER it*. One direction of
one mention problem, by authors who demonstrably knew the problem existed — and the defect
lived in the untested direction.

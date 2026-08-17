---
id: 19e7142a206fa95a
kind: bug
status: fixed
title: 'BUG: the IL3 warn-hook''s unbounded-LHS regex lists ls/cat/find/grep/git — the commands its own warning text calls bounded — so it fires on every legal bounded pipe and contradicts itself in one message'
tags:
- iron-law
- il3
- companion-plugin
- hooks
- false-positive
- cross-repo
closed: 2026-08-17
fix_repo: claude-plugins
opened: 2026-08-17
owner: marius
related:
- docs/trackers/codescout-usage-frictions.md
severity: low
---

## Summary

`codescout-companion`'s IL3 advisory hook decides "unbounded LHS" from one flat regex
alternation that includes `ls`, `cat`, `find`, `grep`, `diff`, `du`, `stat` and an
unconditional `git`. The server's IL3 gate treats all of those as **bounded** and allows
piping them. So the hook fires on every legal bounded-LHS pipe — and its own warning text,
in the same message, names `ls` and `cat` as commands that pass through.

Advisory-only: nothing is blocked, nothing is lost. The cost is that IL3 warnings become
noise, which is exactly how a reader learns to skip the ones that matter.

**The fix lands in a sibling repo** — `claude-plugins`, not codescout. Filed here because
this is where the friction is met and where the IL3 predicate it approximates lives (U-44,
`docs/trackers/codescout-usage-frictions.md`).

## Symptom (Effect)

Reproduced three times, 2026-08-17, on `experiments` at `671e114b`. The clearest case is
also the smallest:

```
run_command("ls docs | head -2")

→ exit_code: 0
  stdout: adrs
          agents

PreToolUse hook additional context:
  IL3 warning — piped `ls docs | head -2` to a log-trimmer.
  …
  codescout's run_command gate already denies unbounded-LHS pipes
  server-side — this hook is an advisory echo, not the enforcer. Run
  bare and query @cmd_xxx; bounded-LHS pipes (ls/cat/awk/sed/find
  -maxdepth N) pass through.
```

The hook flags `ls | head` and then, four lines later in the same block, lists `ls` among
the commands that pass through. The server allowed the command.

Also observed on `git log -3 --format='%s%n%b%n===' | tail -30` and
`git log -3 --format='%h' | tail -1` — both permitted by the rule, since `-3` is an output
limiter; both warned; both ran.

Tally for the session: **3 warnings, 3 legal pipes, 0 true positives.**

## Reproduction

Any session with `codescout-companion` active (plugin `1.16.8`).

1. `run_command("ls docs | head -2")`
2. Observe `exit_code: 0` **and** an IL3 warning in the same tool result.
3. Now a genuine violation — unbounded LHS, no output limiter. Measured this session:

```
run_command("git show HEAD:src/librarian/tools/find.rs | grep -n 'rel_path' | head -60")

→ IL3 violation — piped `git show HEAD:… | grep -n … | head -60` to a log-trimmer. BLOCKED.
   The @cmd_* buffer system saves context tokens: …
```

4. Compare the two. The server **refused** step 3 and **allowed** step 1; the hook emitted
   the same advisory for both. The warning therefore carries no information about which of
   the two you did — which is the whole defect.

(`git show <rev>:<file>` has no output limiter, so it is unbounded under the rule; `git log
-3` and `git status --short` are not. The hook cannot tell those apart either.)
## Environment

`codescout-companion` plugin, version `1.16.8`,
`hooks/il3-warn-hook.mjs`, `PreToolUse` on `mcp__codescout__run_command`.
Repo: `/home/marius/work/claude/claude-plugins` (branch `main`).
Observed from codescout, branch `experiments`.

## Root cause

Measured 2026-08-17: the three commands above, then the hook read at
`claude-plugins/codescout-companion/hooks/il3-warn-hook.mjs:23`.

```js
const LHS = '(cargo|npm|pnpm|yarn|python|pytest|go|mvn|gradle|git|find|ls|grep|cat|diff|du|stat|rg|fd)';
```

One alternation, no bounded set, no limiter inspection. It conflates three different
categories:

| In the regex | Server's classification |
|---|---|
| `cargo npm pnpm yarn python pytest go mvn gradle rg fd` | unbounded — correct |
| `ls cat diff du stat` | **bounded** — allowed to be piped |
| `find` | unbounded only *without* `-maxdepth N` |
| `grep` | unbounded only when recursive (`-r`) |
| `git` | unbounded only *without* an output limiter |

The server's rule, quoted from its own refusal text:

> *"Bounded LHS (ls, cat, stat, du, diff, awk, sed, non-recursive grep) is allowed … Only
> unbounded LHS (cargo, npm, pytest, rg, fd, grep -r, bare find, …) piped to a trimmer is
> blocked. `git` is unbounded ONLY without an output limiter: `git log -3`, `git status
> --short`, `git show --stat` are bounded and may be piped; `--oneline` is not a limiter
> (it bounds width, not line count)."*

Every conditional in that specification — the `-maxdepth` carve-out, the `-r` carve-out,
the git limiter list, the `--oneline` exception — is absent from the hook. The hook's
message describes the server's predicate; the hook's regex implements a different one.

**The class this belongs to.** Two implementations of one rule, only one of them
maintained. The server owns the predicate with its carve-outs; the hook carries a
hand-copied approximation that was never updated as the carve-outs were added. U-22 in
`docs/trackers/codescout-usage-frictions.md` is the same detector producing a different
false positive (a literal `|` inside a `git commit -m` string), which is the tell that the
duplication, not either regex, is the defect.

## Evidence

### The regex, and the message it ships with

Hook line 23 is quoted above. Lines 43-45 of the same file are the message text:

```
codescout's run_command gate already denies unbounded-LHS pipes
server-side — this hook is an advisory echo, not the enforcer.
bare and query @cmd_xxx; bounded-LHS pipes (ls/cat/awk/sed/find
-maxdepth N) pass through.
```

`ls` and `cat` appear in both the flag list (23) and the pass-through list (45).

### The hook concedes it is not the enforcer

Line 43-44, verbatim: *"codescout's run_command gate already denies unbounded-LHS pipes
server-side — this hook is an advisory echo, not the enforcer."* This is the strongest
argument for the fix in *Fix* §3: an echo that disagrees with its source is worse than no
echo, because the source is already correct and already speaks.

## Hypotheses tried

1. **Hypothesis:** the warning is correct and the server is the lenient one — i.e. `ls |
   head` really is an IL3 violation that the server fails to catch.
   **Test:** read the server's refusal text (which states the bounded set explicitly) and
   ran a genuine violation for contrast.
   **Verdict:** rejected. The server's own gate text names `ls` and `cat` as bounded and
   allowed. The hook is the one out of step, and the hook says so itself.
   **Evidence link:** *The hook concedes it is not the enforcer*.

2. **Hypothesis:** only `git` is mishandled — the missing limiter check — and the rest of
   the list is fine.
   **Test:** ran `ls docs | head -2`, which involves no `git` at all.
   **Verdict:** rejected. It warned. The `git` limiter gap is real but is one of five
   categories the flat list collapses; scoping the fix to `git` would leave `ls`, `cat`,
   `diff`, `du`, `stat`, `find -maxdepth`, and non-recursive `grep` still firing.
   **Evidence link:** Symptom.

## Fix

**Shipped 2026-08-17 as option 3 — the predicate is deleted, not corrected.**
User chose this over §1+§2 when the two cases were laid out.

- `claude-plugins:a989d73` — `hooks/il3-warn-hook.mjs` deleted; the
  `mcp__.*__run_command` matcher removed from `hooks/hooks.json`;
  `tests/test-il3-warn-hook.sh` deleted; `il3-warn-hook.mjs` dropped from the
  preserved-hooks list in `tests/test-hooks-json-registration.sh`.
- `claude-plugins:c6fdd64` — version 1.16.8 → 1.16.9 via `scripts/release.sh`
  (canonical + README + `check-versions.sh` gate + cache seeding + install-record
  repoint across all three profiles).
- `claude-plugins:478bc7d` — version-bump-checklist refreshed.

Branch `main`, **local only** (`NO_PUSH=1`), per this file's own Resume note that
codescout sessions do not own that branch.

The argument that settled it: the hook is `contextPreToolUse`, so it can never
block. On a real violation the server already refuses with a better message
carrying the `@cmd_*` recovery path — the hook adds nothing. On a legal pipe the
server is silent and the hook is simply wrong. There is no case where it helps.
Correcting the regex would have rebuilt the duplicated predicate that produced
this bug and U-22; `path_security.rs` is now the sole implementation.

`il3-deny-hook.sh` and its 33-case suite are untouched and still unwired, as they
have been since the 2026-07 downgrade to warn-only. The `mcp__.*__run_command`
matcher now carries no companion hook at all.

Verified at the bytes in all three seeded caches: warn hook absent, deny hook
present, zero `run_command` matchers in `hooks.json`. `run-all.sh` green before
and after.
## Tests added

None — and one **deleted**, which is the finding worth keeping.

This section previously said *"the warn hook has no test"*. That was wrong:
`tests/test-il3-warn-hook.sh` existed, 134 lines, and ran green in every suite
pass. It could not have caught this defect because it **asserted the false
positives as intended behaviour**:

```bash
# New: ls, grep, cat, diff, du
for cmd in "ls -la | head" "grep -r foo src/ | head -50" \
           "cat file.log | tail -20" "diff a b | head" "du -sh */ | sort"; do
  ... pass "fires on: $cmd"
```

Every one of those is bounded under the server's rule and allowed. Same for
`git status --short | grep M` — `--short` *is* an output limiter. The suite was
written from the hook's regex rather than from the server's predicate, so it
locked the divergence in place and made the hook look tested.

That is why the planned six-case matrix below was never written against it: the
file it would have gone in already claimed the opposite. **A test derived from
the implementation it guards cannot detect that the implementation disagrees
with its source of truth.** Nothing was inverted — with the predicate gone there
is no behaviour left to assert, so the suite was deleted with it.

The planned matrix is preserved here as the specification anyone re-promoting
`il3-deny-hook.sh` must satisfy:

- `ls docs | head -2` → no warning
- `cat Cargo.toml | grep name` → no warning
- `git log -3 | tail -1` → no warning
- `git log --oneline | tail -1` → **warns** (guards `--oneline`-is-not-a-limiter)
- `cargo test 2>&1 | grep FAILED` → warns (the only true positive in the set)
- `find . -name '*.rs' | head` → warns; `find . -maxdepth 1 -name '*.rs' | head`
  → no warning
## Workarounds

Ignore the warning when the LHS is bounded — but read the command first, because the
warning is indistinguishable from a true positive. The server is authoritative: if the call
returned output, IL3 did not fire, whatever the advisory said. A refusal, not a warning, is
the real gate.

Do **not** rewrite a working bounded pipe into a bare call plus a buffer query to satisfy
the advisory; that is two round-trips spent on a false positive.

## Resume

One step remains and it is not scriptable: **cold-restart all three Claude Code
instances** (or `/reload-plugins` per instance). Hooks resolve `installPath` at
launch, so a resume is not enough — until then a running session still loads the
1.16.8 cache and will keep emitting the warning.

The three `claude-plugins` commits are **local on `main`, not pushed**
(`NO_PUSH=1`). Push is the maintainer's call.
## References

- `docs/trackers/codescout-usage-frictions.md` — U-44 (this friction), U-22 (the same
  detector, different false positive), U-41 (why a crying-wolf advisory is a real cost),
  U-14 (a companion-plugin matcher citing tools that do not exist).
- `claude-plugins/codescout-companion/hooks/il3-warn-hook.mjs:23` — the regex.
- `claude-plugins/codescout-companion/hooks/il3-deny-hook.sh`,
  `hooks/il3-deny-hook.test.sh` — the sibling hook and the only existing test surface.
- `claude-plugins/docs/trackers/version-bump-checklist.md` — the cache/version trap.
- `docs/architecture/companion-plugin.md` — hook inventory and cross-repo flow.

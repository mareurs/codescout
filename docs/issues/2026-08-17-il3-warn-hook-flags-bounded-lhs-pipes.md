---
id: '87879219a6504b33'
kind: bug
status: open
title: 'BUG: the IL3 warn-hook''s unbounded-LHS regex lists ls/cat/find/grep/git — the commands its own warning text calls bounded — so it fires on every legal bounded pipe and contradicts itself in one message'
tags:
- iron-law
- il3
- companion-plugin
- hooks
- false-positive
- cross-repo
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

Not yet implemented. In `claude-plugins/codescout-companion/hooks/il3-warn-hook.mjs`:

1. **Split the list.** `UNBOUNDED = (cargo|npm|pnpm|yarn|python|pytest|go|mvn|gradle|rg|fd)`,
   plus `grep` only with `-r`/`-R`, plus `find` only without `-maxdepth`. Everything else
   is bounded and must not warn.
2. **Add the git limiter carve-out.** Bounded when the command carries `-N`, `-n`,
   `--max-count`, `--porcelain`, `--short`, or `--stat`; unbounded otherwise. `--oneline`
   is explicitly *not* a limiter.
3. **Preferred over 1 and 2: delete the predicate.** The hook's own text says the server
   is the enforcer and already refuses the real violations with a better message —
   including the `@cmd_*` recovery path. A duplicated predicate is what produced this bug
   and U-22. If the advisory has value it is as an *echo of the server's verdict*, not as
   an independent guess made before the server has spoken.

**Version-bump trap — read before verifying any fix.** Each of the three profiles resolves
plugins from its own version-keyed cache, so editing the hook in the source repo changes
behavior in **none** of them until `.claude-plugin/plugin.json` `version` is bumped (now
`1.16.8`) *and* the install records are refreshed in all three of `~/.claude`,
`~/.claude-sdd`, `~/.claude-kat` (per the global CLAUDE.md rule). A content-only edit
verifies as "no change" and reads as a failed fix. The existing
`claude-plugins/docs/trackers/version-bump-checklist.md` covers this.

## Tests added

None yet. `hooks/il3-deny-hook.test.sh` already exists in that repo and is the natural
home; the warn hook has no test.

Planned, each stated as the pipe and the expected verdict:

- `ls docs | head -2` → no warning. RED today.
- `cat Cargo.toml | grep name` → no warning. RED today.
- `git log -3 | tail -1` → no warning. RED today.
- `git log --oneline | tail -1` → **warns**. Guards the `--oneline`-is-not-a-limiter rule;
  a careless "just allow git" fix breaks this one.
- `cargo test 2>&1 | grep FAILED` → warns. Must stay green throughout — it is the only
  true positive in the set, and the one the hook exists for.
- `find . -name '*.rs' | head` → warns; `find . -maxdepth 1 -name '*.rs' | head` → no
  warning.

Mutation check: restore the flat `LHS` alternation and confirm the first three go red while
the `cargo` case stays green.

## Workarounds

Ignore the warning when the LHS is bounded — but read the command first, because the
warning is indistinguishable from a true positive. The server is authoritative: if the call
returned output, IL3 did not fire, whatever the advisory said. A refusal, not a warning, is
the real gate.

Do **not** rewrite a working bounded pipe into a bare call plus a buffer query to satisfy
the advisory; that is two round-trips spent on a false positive.

## Resume

Work in `claude-plugins` (branch `main` — confirm with the user before committing there;
this repo's sessions do not own that branch). Implement *Fix* §3 if the user agrees the
advisory should defer to the server, otherwise §1 + §2. Write the six cases above into
`hooks/il3-deny-hook.test.sh` (or a sibling `il3-warn-hook.test.sh`) and watch the first
three fail against the current regex before touching line 23.

Then bump `.claude-plugin/plugin.json` and refresh install records in all three profiles —
without that, a verification pass will show the old behavior and read as a failed fix.

## References

- `docs/trackers/codescout-usage-frictions.md` — U-44 (this friction), U-22 (the same
  detector, different false positive), U-41 (why a crying-wolf advisory is a real cost),
  U-14 (a companion-plugin matcher citing tools that do not exist).
- `claude-plugins/codescout-companion/hooks/il3-warn-hook.mjs:23` — the regex.
- `claude-plugins/codescout-companion/hooks/il3-deny-hook.sh`,
  `hooks/il3-deny-hook.test.sh` — the sibling hook and the only existing test surface.
- `claude-plugins/docs/trackers/version-bump-checklist.md` — the cache/version trap.
- `docs/architecture/companion-plugin.md` — hook inventory and cross-repo flow.

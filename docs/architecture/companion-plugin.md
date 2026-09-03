# Companion Plugin: codescout-companion

codescout ships with a companion Claude Code plugin at
**`../claude-plugins/codescout-companion/`** that is **always active** when working
on codescout. `CLAUDE.md` carries only the one critical behavioral fact (native
file/shell tools on source are hard-denied — use codescout MCP tools); the full
hook inventory lives here. Source of truth is the plugin's own `hooks/hooks.json`.

## What it does (headline hooks)

- `SessionStart` hook (`hooks/session-start.mjs`) — injects tool guidance + memory hints into every session; also emits a tracker-hygiene overdue nudge (reads `next-sweep-due` from `docs/trackers/tracker-hygiene-log.md`)
- `SubagentStart` hook (`hooks/subagent-guidance.mjs`) — same for all subagents
- `PreToolUse` hook on `Grep|Glob|Read|Bash|Edit|Write` (`hooks/pre-tool-guard.mjs`) — **hard-denies (`permissionDecision: deny`) native Read/Grep/Glob/Edit/Write on source files and native Bash**, redirecting to codescout MCP tools

## Full hook inventory (per `hooks/hooks.json`)

Re-derived from the installed `hooks/hooks.json` on 2026-08-18 at companion **1.16.9**, because
every row of the previous version of this table was wrong: it named all twelve hooks with `.sh`
extensions the 1.14.0 cross-platform port had replaced with `.mjs`, it gave `pre-task-hint`'s
matcher as `Task` where it is `Agent`, and it omitted five hooks and two whole events. Re-derive it
rather than trusting it:

```
python3 -c "
import json,re
d=json.load(open('<profile>/plugins/cache/sdd-misc-plugins/codescout-companion/<ver>/hooks/hooks.json'))
for ev,es in (d.get('hooks') or d).items():
    print('==',ev)
    for e in es:
        for h in e.get('hooks',[]):
            print('  ',e.get('matcher','-'),'->',','.join(re.findall(r'hooks/([\w.-]+)',json.dumps(h))))
"
```

**SessionStart:**
- `session-start.mjs` — injects tool guidance + memory hints into every session; also emits a tracker-hygiene overdue nudge (reads `next-sweep-due` from `docs/trackers/tracker-hygiene-log.md`).

**SubagentStart:**
- `subagent-guidance.mjs` — the same guidance for every subagent.

**UserPromptSubmit:**
- `constitution-brief.mjs` — buddy-constitution surface. Also the hook that resolves `/buddy:summon` before the slash command runs, spilling an oversized persona payload to a guard-exempt file under `.buddy/<sid>/` and injecting a pointer.

**PreCompact:**
- `constitution-epoch-bump.mjs` — bumps the constitution epoch across a compaction; observed firing in this session's `/compact`.

**PreToolUse (guards — hard `permissionDecision: deny`):**
- `mcp__codescout__(edit_code|edit_file|edit_markdown|create_file)` → `worktree-write-guard.mjs` — blocks codescout write tools when in a git worktree until `workspace(activate)` has run (clears the `.cs-worktree-pending` marker).
- `Edit|Write|mcp__codescout__(edit_code|edit_file|create_file)` → `constitution-guard.mjs` — buddy-constitution write guard. Note the matcher is **not** the same set as `worktree-write-guard`'s: it adds native `Edit`/`Write` and omits `edit_file`.
- `Grep|Glob|Read|Bash|Edit|Write` → `pre-tool-guard.mjs` — **hard-denies native Read/Grep/Glob/Edit/Write on source files and all native Bash**, redirecting to codescout MCP tools.
- `Bash` → `git-worktree-guard.mjs` — denies worktree-ambiguous destructive git verbs from Bash; requires `git -C <path>` (single-worktree repos carved out).
- ~~`mcp__.*__read_file` → `il4-deny-hook.mjs`~~ — **RETIRED 2026-09-03; no longer installed.** It hard-denied `read_file` on `.md` paths and redirected to `read_markdown`, which the 2026-09-02 fold retired — so the redirect named a tool the server no longer registers, and `read_file` *is* the heading-addressed markdown reader now. Deleted from the plugin at `claude-plugins:bb24b7f` (patch-id `1175e6f8b54fff099d6342967d4ea09b8f92a6a4`) and shipped in **1.20.4**, the version installed in all three profiles; the `1.20.3 → 1.20.4` diff removes exactly `il4-deny-hook.mjs` and its test. Probed 2026-09-04 01:33 EEST: `read_file` on a `.md` path reaches the server and returns a heading map, no deny. **Struck through rather than deleted, because this line asserted *"It still fires (observed this session)"* for ~41h after the hook was gone** — the inventory is the surface a reader trusts, and a silent deletion would leave nothing to warn the next reader that it can go stale. History: `docs/issues/archive/2026-09-03-il4-deny-hook-will-deadlock-markdown-reads-after-the-fold.md` (artifact `13382b706c9c77b0`); the class is `observer-blindness:OB-16`.

**PreToolUse (advisory — `exit 0` + injected hint):**
- `Agent` → `pre-task-hint.mjs` — on the first subagent dispatch of a session, points at the `reconnaissance` skill. **The matcher is `Agent`, not `Task`** — this doc claimed `Task` for months, which is the tool name that no longer exists.
- `Agent` → `explore-inject.mjs` — second hook on the same matcher; ships with `explore-inject.fixtures.jsonl`.
- `mcp__codescout__edit_code` → `pre-edit-hint.mjs` — on the first shape-changing edit of a session, points at recon-for-shape-changes.
- `mcp__.*__run_command` → **no hook, since companion 1.16.9.** IL3 is enforced entirely server-side by codescout's `src/util/path_security.rs`, which *blocks* an unbounded-LHS pipe and emits the `@cmd_*` recovery path. The advisory `il3-warn-hook.mjs` was deleted in `claude-plugins:a989d73`, not corrected: as a `contextPreToolUse` hook it could never block, so it was redundant whenever the server refused and simply wrong whenever the server allowed — and its hand-copied regex listed `ls`/`cat`/`find`/`grep`/`git` as unbounded, the commands its own warning text called bounded, so it fired on every legal bounded pipe (U-44, `docs/issues/archive/2026-08-17-il3-warn-hook-flags-bounded-lhs-pipes.md`). `il3-deny-hook.sh` and its suite (58 cases as of 2026-08-27) remain on disk and unwired, kept for possible re-promotion; the matcher itself now carries nothing. **Do not "restore" an advisory mirror of a server-side predicate** — that duplication is what produced U-22 and U-44. **Parked is not abandoned, though:** `bb85c55` and `5f6b336` both synced this file to server-side changes while it was already unwired, and `claude-plugins:88f1e29` synced it again to codescout `18f8f9d1` (field selectors, the collapse rule, single-line git plumbing). A dormant mirror that silently rots is worse than no mirror, because re-promotion then reintroduces measured false positives — 19 of 703 refusals, in `18f8f9d1`'s case. The file now carries a `STATUS: PARKED AND UNWIRED` header naming its one remaining known divergence (it does not split on `;`/`&&`, so `PRE_PIPE` is everything before the first pipe in the whole command), which a re-promotion must fix before wiring.

**PostToolUse (state sync):**
- `EnterWorktree` → `worktree-activate.mjs` — injects workspace guidance, drops the `.cs-worktree-pending` write-block marker, symlinks `.codescout/` into the worktree.
- `mcp__.*__workspace` → `cs-activate-project.mjs` — records the declared workspace (statusline) and removes `.cs-worktree-pending` (unblocks write tools).

**Stop:**
- `goal-stop-hook.mjs` — queries codescout goal-tracker artifacts at turn end and surfaces refresh-staleness in the stop reason; fail-open; disable via `.claude/codescout-companion.json {"goal_stop_hook": false}`.

**Not hooks, despite living in `hooks/`:** `detect-tools.sh` and `detect.mjs` are sourced/imported libraries, `lib.mjs` is shared code, and every `*.test.sh` / `*.fixtures.jsonl` is a test. `hooks.json` is the only authority for what is wired.
## Critical implication for working on this codebase

The `PreToolUse` hook will **block** any attempt to use native `Read`, `Grep`, or `Glob` on source files (`.rs`, `.ts`, `.py`, etc) and **all native `Bash`**. You will see a `PreToolUse` hook deny. **Use codescout's MCP tools instead:**

- `symbols(path)` — all symbols in a file/dir
- `symbols(name=..., include_body=true)` — read a function body
- `grep(pattern)` — regex search
- `semantic_search(query)` — concept-level search
- `read_file(path)` — for non-source files (toml, json); `read_file(path)` for `.md`
- `run_command(command)` — shell, cwd sandboxed to the active project

## Cross-repo work (companion: hardened 2026-05-21)

The Bash branch of `pre-tool-guard.mjs` no longer allows a `cd`-escape. **All native `Bash` is hard-denied and redirected to `run_command`**, whose cwd is sandboxed to the active project. For a sibling repo's git, run from the project root via `run_command(command="git -C /abs/path <subcommand>")` — no `cd` needed. For non-git work in a sibling (or out-of-shape commands like `pushd` / `bash -c '...'`), switch the codescout workspace explicitly:

```
workspace(action="activate", path="/path/to/sibling", read_only=false)
# ...do the work...
workspace(action="activate", path="<absolute path of THIS repo>", read_only=false)
```

## Concurrent multi-workspace: one server, one active project

The codescout MCP server holds a single active project at a time — **any**
`workspace(activate)` call, whether from the top-level session or from a subagent,
replaces it for every caller sharing that server, with no notice to a call already in
flight. **When briefing a subagent — an `Agent` dispatch, or an `agent()` call inside a
`Workflow` script — to work in a different repo, tell it to pass `workspace=<abs path>`
on each call. Never brief it to call `workspace(action="activate")` itself**; that is
the one form that mutates the shared state every other concurrent caller resolves
against.

This is not hypothetical. `docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`
traces a live incident where exactly this mistake, in a `Workflow` script's subagent
prompt, broke the parent session's own writes mid-turn with a misleading "read-only"
error — three separate times in one session, all recovered after the fact, none
prevented in advance.

After any `workspace(activate, path=foreign)` issued by the top-level session itself,
restore the home project before finishing the turn. Full rules: `get_guide("workspace-state")`.

---
status: fixed
opened: 2026-05-28
closed: 2026-05-28
severity: low
owner: marius
related:
  - U-23
  - docs/trackers/codescout-usage-frictions.md
tags:
  - prompt-surface
  - usage-friction
  - worktree
kind: bug
---

# BUG: `[codescout] paths are relative to …` annotation spams every tool call; activation + worktree state invisible

## Summary
The U-23 fix (2026-05-25) made the path-disambiguation annotation fire on
every stripped tool response — ~50 bytes per call, multiplied by every
non-`run_command` tool. In real sessions inside a git worktree
(`/home/marius/work/mirela/backend-kotlin/.worktrees/weekly-pattern`) the
repetition is loud, while the actual signals the user wants — "project was
activated" and "you are in a worktree" — were nowhere in the visible
session state. This file documents the brainstormed redesign that
replaces per-call emission with novelty-gated emission, plus two new
banners in `build_server_instructions` that carry the cold-reader signal
that U-23 was protecting.

## Symptom (Effect)
After every non-`run_command` tool call whose output contained the
project root prefix:

```
[codescout] paths are relative to /home/marius/work/mirela/backend-kotlin/.worktrees/weekly-pattern
```

Same string, every call. Useful the first time; noise the next twenty.
Meanwhile, no surface signal that `activate_project` had happened (it
auto-detects from CWD at stdio startup), and no signal that the active
project is a linked worktree rather than the main checkout.

## Reproduction
Pre-fix commit: `git log --grep "U-23"` shows the cadence change.
Pre-fix shape:

```
make_server() ; for each tool : post_process → annotation
```

Every iteration appended `Content::text("\n[codescout] paths are
relative to …")`.

## Environment
- codescout `experiments` branch, this session (2026-05-28)
- stdio MCP transport (Claude Code launch)
- Project: any git worktree; observed in
  `/home/marius/work/mirela/backend-kotlin/.worktrees/weekly-pattern`

## Root cause
Three independent gaps surfaced together:

1. **`src/server.rs:341-393` (`post_process`)** emitted the annotation on
   every stripped response. Per-session cap was removed by U-23 because
   capped-then-silent left cold readers (post-compaction) with no path
   context for late tool calls. The fix solved correctness at the cost of
   spam.

2. **`src/prompts/mod.rs:27` (`build_server_instructions`)** carried no
   explicit "this project is activated" marker — the line
   `**Project:** <name> at <path>` reads as a passive label rather than
   an active-state signal.

3. **No worktree detection anywhere.** Agents had to infer worktree vs
   main checkout from path shape (a `.worktrees/` substring). When CWD
   diverges from the main repo at launch time, the state is silent.

## Evidence
The U-23 docstring (`src/server.rs:359-369` pre-fix) explicitly cited the
correctness rationale ("a fresh reader cold-reads later context misread
stripped paths as raw catalog data"). U-24 (2026-05-25) added a related
docstring-vs-runtime note. Both are referenced in the new gate's doc
comment.

## Hypotheses tried
1. **Suppress the annotation entirely once per session.** Rejected —
   regresses U-23.
2. **Shorten the annotation to a sigil (`[~codescout~]`).** Rejected —
   trades correctness-by-redundancy for terse-by-convention, requires
   prompt surface change to teach the convention.
3. **Move the signal to `server_instructions` so it survives compaction,
   then gate the per-response annotation by novelty.** Accepted —
   `build_server_instructions` already refreshes on `activate_project`
   via `refresh_instructions` at `src/server.rs:387`, so the explicit
   activation banner + worktree line are free to add and carry the
   cold-reader signal without per-call cost.

## Fix
Three coordinated edits on `experiments` this session (master SHA to be
recorded after cherry-pick — see CLAUDE.md § "After cherry-pick"):

1. **A — novelty-gated annotation** (`src/server.rs:71-88`,
   `src/server.rs:341-393`, `src/server.rs:790-791`). Repurposed the
   vestigial `_path_note_count: AtomicUsize` field into
   `path_note_emitted_since_activation: AtomicBool`. `post_process`
   emits the annotation only on the first stripped response since
   server start or last `activate_project`. The activation branch of
   `call_tool` (`src/server.rs:781-791`) resets the bool to `false`
   so the next stripped call re-emits with the new root.

2. **C — worktree-aware validation** (`src/prompts/mod.rs:114-172`).
   Added `WorktreeInfo` struct + `detect_worktree_info` (filesystem-only;
   reads `.git` as a file, parses `gitdir:` + `<gitdir>/HEAD`). Wired
   through `ProjectStatus` (`src/prompts/mod.rs:194-213`) and populated
   in `Agent::project_status` (`src/agent/mod.rs:721-771`).
   `build_server_instructions` renders
   `**Worktree:** branch \`<branch>\` of \`<main_repo>\``  when present.

3. **D — explicit activation banner** (`src/prompts/mod.rs:27-118`).
   `**Project:**` → `**Active project:**` so launch-time auto-activation
   is visible. Refreshes on every `activate_project` via the existing
   `refresh_instructions` path.

## Tests added
- `prompts::tests::build_with_project_appends_status` — updated to
  assert the new `**Active project:**` wording and absence of any
  `Worktree:` line on non-worktree projects.
- `prompts::tests::build_with_worktree_emits_worktree_banner` — asserts
  the worktree line is rendered with branch + main_repo when
  `ProjectStatus.worktree` is `Some(...)`.
- `prompts::tests::build_with_detached_worktree_renders_placeholder` —
  asserts `<detached HEAD>` placeholder when `branch: None`.
- `prompts::tests::detect_worktree_info_identifies_linked_worktree` —
  filesystem fixture: writes a `<tmp>/main/.git/worktrees/feat/HEAD`
  + `<tmp>/wt/.git` pointer, expects detection to return correct
  branch + main_repo.
- `prompts::tests::detect_worktree_info_returns_none_for_regular_checkout` —
  `.git/` as a directory must return `None`.
- `prompts::tests::detect_worktree_info_returns_none_when_no_git` —
  defensive: no `.git` at all, no panic.
- `server::tests::stripped_responses_emit_paths_relative_annotation_once_per_activation`
  (renamed from `..._always_carry_...`) — first stripped response gets
  the annotation; subsequent stripped responses across 5 tool names do
  not; manual reset of the bool restores single-shot emission. The
  `run_command` negative branch (raw bytes never annotated) kept
  unchanged.

`cargo test --lib`: 2528 passed, 0 failed, 7 ignored.
`cargo clippy --lib --tests -- -D warnings`: clean.
`cargo fmt`: clean.

## Workarounds
N/A — landed this session.

## Resume
N/A — fixed. Follow-up watchpoints:
- If U-23-class cold-reader regressions resurface (paths misread as
  catalog data inside a compacted transcript that ALSO lost the
  `server_instructions` block), revisit whether the activation banner is
  surviving compaction in practice or whether we need a per-buffer
  fallback annotation.
- The `worktree.main_repo` field is filesystem-derived — if a user moves
  the main repo dir without updating the worktree's `gitdir:` pointer,
  the banner will show the stale path. Detect this with `Path::exists`
  on `main_repo` if it bites in practice.

## References
- U-23 entry: `docs/trackers/codescout-usage-frictions.md` § U-23
- U-24 entry: same file § U-24 (the docstring-vs-runtime follow-up)
- `src/server.rs:71-88` — gate field + doc comment
- `src/server.rs:341-393` — `post_process` body
- `src/server.rs:781-791` — activation reset
- `src/prompts/mod.rs:114-172` — `WorktreeInfo` + `detect_worktree_info`
- `src/prompts/mod.rs:27-118` — `build_server_instructions` (banners)
- `src/agent/mod.rs:721-771` — `project_status` populates worktree
